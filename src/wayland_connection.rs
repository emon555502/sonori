// unused for now

use std::time::Duration;

use copypasta::{ClipboardContext, ClipboardProvider};
use smithay_client_toolkit::{
    delegate_registry, delegate_seat,
    reexports::client::{
        globals::registry_queue_init,
        protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
        Connection, Dispatch, Proxy, QueueHandle,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
};
use std::ops::RangeInclusive;
use std::process::Command;
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{Event as TextInputEvent, ZwpTextInputV3},
};

pub struct WaylandConnection {
    connection: Connection,
    queue: wayland_client::EventQueue<WaylandState>,
    state: WaylandState,
    clipboard: ClipboardContext,
}

// Define the TextInputState to track text input state
pub struct TextInputState {
    text_input_manager: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,
    focused_surface: Option<String>,
}

impl TextInputState {
    pub fn new() -> Self {
        Self {
            text_input_manager: None,
            text_input: None,
            focused_surface: None,
        }
    }

    // Check if an input field is focused
    pub fn is_input_field_focused(&self) -> bool {
        self.focused_surface.is_some()
    }
}

// Define TextInputHandler trait for handling text input events
pub trait TextInputHandler {
    fn text_input_state(&mut self) -> &mut TextInputState;

    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _text_input: &ZwpTextInputV3,
        _surface: &WlSurface,
    ) where
        Self: Sized,
    {
        println!("Entering text input");
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _text_input: &ZwpTextInputV3,
        _surface: &WlSurface,
    ) where
        Self: Sized,
    {
        println!("Leaving text input");
    }

    fn done(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _text_input: &ZwpTextInputV3)
    where
        Self: Sized,
    {
        println!("Done text input");
    }
}

// Create a delegate macro for text input events
macro_rules! delegate_text_input {
    ($state:ty) => {
        impl Dispatch<ZwpTextInputV3, ()> for $state {
            fn event(
                state: &mut Self,
                text_input: &ZwpTextInputV3,
                event: TextInputEvent,
                _data: &(),
                conn: &Connection,
                qh: &QueueHandle<Self>,
            ) {
                match event {
                    TextInputEvent::Enter { surface } => {
                        TextInputHandler::enter(state, conn, qh, text_input, &surface);
                    }
                    TextInputEvent::Leave { surface } => {
                        TextInputHandler::leave(state, conn, qh, text_input, &surface);
                    }
                    TextInputEvent::Done { serial: _ } => {
                        TextInputHandler::done(state, conn, qh, text_input);
                    }
                    _ => {} // Ignore other events for now
                }
            }
        }

        impl Dispatch<ZwpTextInputManagerV3, ()> for $state {
            fn event(
                _state: &mut Self,
                _manager: &ZwpTextInputManagerV3,
                _event: <ZwpTextInputManagerV3 as wayland_client::Proxy>::Event,
                _data: &(),
                _conn: &Connection,
                _qh: &QueueHandle<Self>,
            ) {
                // The text input manager has no events
            }
        }
    };
}

pub struct WaylandState {
    registry_state: RegistryState,
    seat_state: SeatState,
    text_input_state: TextInputState,
    seat: Option<WlSeat>,
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers!();
}

impl SeatHandler for WaylandState {
    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: WlSeat) {
        self.seat = Some(seat);
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {
        self.seat = None;
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => {
                self.seat = Some(seat);
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => {
                self.seat = None;
            }
            _ => {}
        }
    }

    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
}

impl TextInputHandler for WaylandState {
    fn text_input_state(&mut self) -> &mut TextInputState {
        &mut self.text_input_state
    }

    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _text_input: &ZwpTextInputV3,
        surface: &WlSurface,
    ) where
        Self: Sized,
    {
        self.text_input_state.focused_surface =
            Some(format!("Surface: {:?}", surface.id().protocol_id()));
        println!(
            "Input field focused on {}",
            self.text_input_state.focused_surface.as_ref().unwrap()
        );
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _text_input: &ZwpTextInputV3,
        _surface: &WlSurface,
    ) where
        Self: Sized,
    {
        println!("Input field lost focus");
        self.text_input_state.focused_surface = None;
    }

    fn done(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _text_input: &ZwpTextInputV3)
    where
        Self: Sized,
    {
        println!("Text input done");
    }
}

delegate_registry!(WaylandState);
delegate_seat!(WaylandState);
delegate_text_input!(WaylandState);

impl WaylandConnection {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect_to_env()?;
        let (globals, mut event_queue) = registry_queue_init(&connection)?;

        let qh = event_queue.handle();
        let registry_state = RegistryState::new(&globals);
        let seat_state = SeatState::new(&globals, &qh);

        let mut state = WaylandState {
            registry_state,
            seat_state,
            text_input_state: TextInputState::new(),
            seat: None,
        };

        // Find first seat
        if let Some(seat) = state.seat_state.seats().next() {
            state.seat = Some(seat.clone());
        }

        // Try to bind text input manager using state.registry_state
        for global in state.registry_state.globals() {
            if global.interface == "zwp_text_input_manager_v3" {
                state.text_input_state.text_input_manager = state
                    .registry_state
                    .bind_one::<ZwpTextInputManagerV3, _, _>(
                        &qh,
                        RangeInclusive::new(1, global.version),
                        (),
                    )
                    .ok();
                break;
            }
        }

        // Initialize text input if available
        if let (Some(seat), Some(text_input_manager)) =
            (&state.seat, &state.text_input_state.text_input_manager)
        {
            let text_input = text_input_manager.get_text_input(seat, &qh, ());
            state.text_input_state.text_input = Some(text_input);

            if let Some(text_input) = &state.text_input_state.text_input {
                // Enable text input events
                text_input.enable();
                // Commit to receive events
                text_input.commit();
            }
        } else {
            println!("Missing required protocols for text input");
            if state.seat.is_none() {
                println!("No seat found");
            }
            if state.text_input_state.text_input_manager.is_none() {
                println!("No text input manager found");
            }
        }

        // Initialize clipboard
        let clipboard = match ClipboardContext::new() {
            Ok(ctx) => ctx,
            Err(e) => return Err("Clipboard initialization failed".into()),
        };

        println!("Successfully connected to Wayland server");
        Ok(Self {
            connection,
            queue: event_queue,
            state,
            clipboard,
        })
    }

    pub fn dispatch_pending(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.queue.dispatch_pending(&mut self.state)?;

        // Re-enable text input periodically to ensure it's active
        if let Some(text_input) = &self.state.text_input_state.text_input {
            text_input.enable();
            text_input.commit();
        }

        Ok(())
    }

    pub fn process_events(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.queue
            .blocking_dispatch(&mut self.state)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        Ok(())
    }

    pub fn dispatch_with_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Note: smithay_client_toolkit doesn't directly support timeout dispatching, so we simulate it
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if self.queue.dispatch_pending(&mut self.state).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10)); // Avoid tight loop
        }
        Ok(())
    }

    pub fn is_input_field_focused(&self) -> bool {
        self.state.text_input_state.focused_surface.is_some()
    }

    /// Send text to the currently focused input field using the clipboard and wtype
    ///
    /// This method:
    /// 1. Copies the text to the clipboard
    /// 2. Uses wtype to simulate Ctrl+V to paste the text
    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        if !self.is_input_field_focused() {
            println!("No input field focused - cannot send text");
            return Err("No input field focused".to_string());
        }

        // Copy text to clipboard with copypasta
        if let Err(e) = self.clipboard.set_contents(text.to_string()) {
            return Err(format!("Failed to copy to clipboard: {}", e));
        }
        println!("Copied '{}' to clipboard", text);

        // Use wtype to simulate Ctrl+V
        match Command::new("wtype")
            .arg("-k")
            .arg("control")
            .arg("-k")
            .arg("v")
            .status()
        {
            Ok(status) => {
                if status.success() {
                    println!("Pasted '{}' successfully", text);
                    Ok(())
                } else {
                    println!("Failed to paste with wtype - is it installed?");
                    Err("wtype failed".to_string())
                }
            }
            Err(e) => {
                println!("Error executing wtype: {}", e);
                Err(format!("Error executing wtype: {}", e))
            }
        }
    }
}
