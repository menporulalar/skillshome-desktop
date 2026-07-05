//! In-memory sign-in state managed by Tauri. The access token lives here only — never
//! written to disk. `SigninStatus` is pushed to the frontend via the `signin://status`
//! event so the UI never has to poll.

use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, serde::Serialize, Default)]
#[serde(tag = "state")]
pub enum SigninStatus {
    #[default]
    Idle,
    AwaitingBrowser,
    AwaitingDeviceConfirmation {
        user_code: String,
        verification_uri: String,
    },
    Success,
    Error {
        message: String,
    },
}

#[derive(Default)]
pub struct SigninState {
    pub access_token: Mutex<Option<String>>,
    pub status: Mutex<SigninStatus>,
}

impl SigninState {
    pub fn set_status(&self, status: SigninStatus) {
        *self.status.lock().unwrap() = status;
    }

    pub fn status(&self) -> SigninStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn set_access_token(&self, token: Option<String>) {
        *self.access_token.lock().unwrap() = token;
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.lock().unwrap().clone()
    }
}
