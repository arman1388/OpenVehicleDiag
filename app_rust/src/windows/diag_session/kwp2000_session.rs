use std::{
    borrow::BorrowMut,
    cell::RefCell,
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
    time::Instant,
};

use iced::{time, Column, Container, Length, Row, Space, Subscription};
use log_view::{LogType, LogView};

use crate::{
    commapi::{
        comm_api::{ComServer, ISO15765Config},
        protocols::{kwp2000::KWP2000ECU, ProtocolServer},
    },
    themes::{button_outlined, text, text_input, title_text, ButtonType, TextType, TitleSize},
    windows::{diag_manual::DiagManualMessage, window},
};

use super::{log_view, DiagMessageTrait, SessionMsg, SessionResult, SessionTrait};

#[derive(Debug, Clone, PartialEq)]
pub enum KWP2000DiagSessionMsg {
    ConnectECU,
    DisconnectECU,
    Back,
    PollServer(Instant),
    LoadErrorDefinition,
    ClearLogs,
    ClearErrors,
    ReadCodes,
    SendPayload,
    EnterPayload(String),
}

impl DiagMessageTrait for KWP2000DiagSessionMsg {
    fn is_back(&self) -> bool {
        self == &KWP2000DiagSessionMsg::Back
    }
}

#[derive(Debug, Clone)]
pub struct KWP2000DiagSession {
    ecu: ISO15765Config,
    server: Box<dyn ComServer>,
    connect_btn: iced::button::State,
    disconnect_btn: iced::button::State,
    back_btn: iced::button::State,
    can_clear_codes: bool,
    clear_btn: iced::button::State,
    read_codes_btn: iced::button::State,
    diag_server: Option<KWP2000ECU>,
    payload_string: String,
    payload_send_btn: iced::button::State,
    payload_input: iced::text_input::State,
    can_send: bool,
    logview: LogView,
}

impl KWP2000DiagSession {
    pub fn new(comm_server: Box<dyn ComServer>, ecu: ISO15765Config) -> SessionResult<Self> {
        Ok(Self {
            ecu,
            server: comm_server,
            connect_btn: Default::default(),
            disconnect_btn: Default::default(),
            back_btn: Default::default(),
            diag_server: None,
            logview: LogView::new(),
            can_clear_codes: false,
            clear_btn: Default::default(),
            read_codes_btn: Default::default(),
            payload_string: String::new(),
            payload_send_btn: Default::default(),
            payload_input: Default::default(),
            can_send: false,
        })
    }
}

impl SessionTrait for KWP2000DiagSession {
    type msg = KWP2000DiagSessionMsg;

    fn view(&mut self) -> iced::Element<Self::msg> {
        let mut ui = Column::new().push(title_text("KWP2000 diagnostic session", TitleSize::P3));

        let in_session = if let Some(ref s) = self.diag_server {
            s.is_in_diag_session()
        } else {
            false
        };

        let display_btn = if in_session {
            button_outlined(
                &mut self.disconnect_btn,
                "Disconnect ECU",
                ButtonType::Warning,
            )
            .on_press(KWP2000DiagSessionMsg::DisconnectECU)
        } else {
            button_outlined(&mut self.disconnect_btn, "Connect ECU", ButtonType::Primary)
                .on_press(KWP2000DiagSessionMsg::ConnectECU)
        };

        ui = ui.push(display_btn);

        if !in_session {
            ui = ui.push(
                button_outlined(&mut self.back_btn, "Back", ButtonType::Secondary)
                    .on_press(KWP2000DiagSessionMsg::Back),
            )
        } else {
            ui = ui.push(
                button_outlined(
                    &mut self.read_codes_btn,
                    "Read error codes",
                    ButtonType::Secondary,
                )
                .on_press(KWP2000DiagSessionMsg::ReadCodes),
            );
            if self.can_clear_codes {
                ui = ui.push(
                    button_outlined(
                        &mut self.clear_btn,
                        "Clear error codes",
                        ButtonType::Secondary,
                    )
                    .on_press(KWP2000DiagSessionMsg::ClearErrors),
                );
            }

            // Payload input
            ui = ui.push(text("Enter payload (Hex string)", TextType::Normal));
            ui = ui.push(text_input(
                &mut self.payload_input,
                "",
                &self.payload_string,
                KWP2000DiagSessionMsg::EnterPayload,
            ));
            let mut btn = button_outlined(
                &mut self.payload_send_btn,
                "Send payload",
                ButtonType::Warning,
            );
            if self.can_send {
                btn = btn.on_press(KWP2000DiagSessionMsg::SendPayload);
            }
            ui = ui.push(btn);
        }
        ui = ui.push(Space::with_height(Length::Fill));
        if let Some(se) = &self.diag_server {
            ui = ui.push(Row::new().push(text(
                format!("Current session type: {:?}", se.get_session_type()).as_str(),
                TextType::Normal,
            )));
        }

        Row::new()
            .spacing(8)
            .padding(8)
            .push(ui.width(Length::FillPortion(1)))
            .push(
                Container::new(self.logview.view(KWP2000DiagSessionMsg::ClearLogs))
                    .width(Length::FillPortion(1)),
            )
            .into()
    }

    fn update(&mut self, msg: &Self::msg) -> Option<Self::msg> {
        match msg {
            KWP2000DiagSessionMsg::ConnectECU => {
                match KWP2000ECU::start_diag_session(self.server.clone(), &self.ecu) {
                    Ok(server) => {
                        window::disable_home();
                        self.diag_server = Some(server);
                        self.logview
                            .add_msg("Connection to ECU established", LogType::Info)
                    }
                    Err(e) => self.logview.add_msg(
                        format!("Error connecting to ECU ({})", e.get_text()),
                        LogType::Info,
                    ),
                }
            }
            KWP2000DiagSessionMsg::DisconnectECU => {
                if let Some(ref mut server) = self.diag_server {
                    server.borrow_mut().exit_diag_session()
                }
                self.logview
                    .add_msg("Connection to ECU terminated", LogType::Info);
                self.diag_server.take();
                window::enable_home();
            }

            KWP2000DiagSessionMsg::PollServer(_) => {
                if let Some(ref mut server) = self.diag_server {
                    if !server.is_in_diag_session() {
                        // Woops server terminated without interaction
                        server.exit_diag_session();
                        self.logview
                            .add_msg("Connection to ECU closed unexpectedly", LogType::Info);
                        if let Some(desc) = server.get_last_error() {
                            self.logview.add_msg(format!("--> {}", desc), LogType::Info);
                        }
                        self.diag_server.take();
                        window::enable_home();
                    }
                }
            }
            KWP2000DiagSessionMsg::ClearLogs => self.logview.clear_logs(),
            KWP2000DiagSessionMsg::ClearErrors => {
                if let Some(s) = &self.diag_server {
                    match s.clear_errors() {
                        Err(e) => self.logview.add_msg(
                            format!("Error clearing ECU errors: {}", e.get_text()).as_str(),
                            LogType::Error,
                        ),
                        Ok(_) => self
                            .logview
                            .add_msg("ECU Errors cleared successfully", LogType::Error),
                    }
                }
            }
            KWP2000DiagSessionMsg::ReadCodes => {
                self.can_clear_codes = false;
                if let Some(s) = &self.diag_server {
                    match s.read_errors() {
                        Err(e) => self.logview.add_msg(
                            format!("Error reading ECU errors: {}", e.get_text()).as_str(),
                            LogType::Error,
                        ),
                        Ok(errors) => {
                            if errors.is_empty() {
                                self.logview.add_msg("No ECU Errors found", LogType::Info)
                            } else {
                                self.logview.add_msg(
                                    format!("Found {} errors", errors.len()),
                                    LogType::Warn,
                                );
                                self.can_clear_codes = true;
                                for x in &errors {
                                    self.logview.add_msg(x.error.as_str(), LogType::Warn);
                                }
                            }
                        }
                    }
                }
            }
            KWP2000DiagSessionMsg::EnterPayload(s) => {
                self.payload_string = s.clone();
                if s.is_empty() {
                    self.can_send = false;
                } else if hex::decode(s).is_ok() && s.len() >= 4 {
                    self.can_send = true;
                }
            }
            KWP2000DiagSessionMsg::SendPayload => {
                if let Ok(r) = hex::decode(&self.payload_string) {
                    if r.len() >= 2 {
                        if let Some(server) = &self.diag_server {
                            match server.run_command(r[0], &r[1..]) {
                                Ok(res) => self.logview.add_log(
                                    format!("Req:  {:02X?}", r),
                                    format!("Resp: {:02X?}", res),
                                    LogType::Info,
                                ),
                                Err(e) => self.logview.add_log(
                                    format!("Req:  {:02X?}", r),
                                    format!("Exec error: {}", e.get_text()),
                                    LogType::Error,
                                ),
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn subscription(&self) -> iced::Subscription<Self::msg> {
        if self.diag_server.is_some() {
            time::every(std::time::Duration::from_millis(250))
                .map(KWP2000DiagSessionMsg::PollServer)
        } else {
            Subscription::none()
        }
    }
}

impl Drop for KWP2000DiagSession {
    fn drop(&mut self) {
        if let Some(ref mut session) = self.diag_server {
            session.exit_diag_session()
        }
    }
}
