/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::{invalid_transition, Event, InternalStateMachine, State};
use crate::{Error, FxaEvent, FxaState, Result};
use error_support::report_error;

pub struct AuthorizingStateMachine;

// Save some typing
use Event::*;
use State::*;

impl InternalStateMachine for AuthorizingStateMachine {
    fn initial_state(&self, event: FxaEvent) -> Result<State> {
        match event {
            FxaEvent::CompleteOAuthScopeAuthorizationFlow { code, state } => {
                Ok(CompleteOAuthScopeAuthorizationFlow { code, state })
            }
            FxaEvent::CancelOAuthFlow => Ok(Complete(FxaState::Connected)),
            // Allow apps to begin a new scope authorization flow when one is already in progress.
            FxaEvent::BeginOAuthScopeAuthorizationFlow { scopes, entrypoint } => {
                Ok(State::BeginOAuthScopeAuthorizationFlow { scopes, entrypoint })
            }
            FxaEvent::Disconnect => Ok(Disconnect),
            e => Err(Error::InvalidStateTransition(format!("Authorizing -> {e}"))),
        }
    }

    fn next_state(&self, state: State, event: Event) -> Result<State> {
        Ok(match (state, event) {
            // On success, go directly to Connected — no InitializeDevice step needed since
            // the device is already initialized.
            (CompleteOAuthScopeAuthorizationFlow { .. }, CompleteOAuthFlowSuccess) => {
                Complete(FxaState::Connected)
            }
            // On failure, return to Connected rather than Disconnected.
            (CompleteOAuthScopeAuthorizationFlow { .. }, CallError) => {
                Complete(FxaState::Connected)
            }
            (BeginOAuthScopeAuthorizationFlow { .. }, BeginOAuthFlowSuccess { oauth_url }) => {
                Complete(FxaState::Authorizing { oauth_url })
            }
            // On failure to begin, stay Connected via Cancel (handled by the outer loop).
            (BeginOAuthScopeAuthorizationFlow { .. }, CallError) => Cancel,
            (Disconnect, DisconnectSuccess) => Complete(FxaState::Disconnected),
            (Disconnect, CallError) => {
                // disconnect() is currently infallible, but let's handle errors anyway in case we
                // refactor it in the future.
                report_error!("fxa-state-machine-error", "saw CallError after Disconnect");
                Complete(FxaState::Disconnected)
            }
            (state, event) => return invalid_transition(state, event),
        })
    }
}

#[cfg(test)]
mod test {
    use super::super::StateMachineTester;
    use super::*;

    #[test]
    fn test_complete_scope_authorization_flow() {
        let tester = StateMachineTester::new(
            AuthorizingStateMachine,
            FxaEvent::CompleteOAuthScopeAuthorizationFlow {
                code: "test-code".to_owned(),
                state: "test-state".to_owned(),
            },
        );
        assert_eq!(
            tester.state,
            CompleteOAuthScopeAuthorizationFlow {
                code: "test-code".to_owned(),
                state: "test-state".to_owned(),
            }
        );
        assert_eq!(
            tester.peek_next_state(CallError),
            Complete(FxaState::Connected)
        );
        assert_eq!(
            tester.peek_next_state(CompleteOAuthFlowSuccess),
            Complete(FxaState::Connected)
        );
    }

    #[test]
    fn test_cancel_oauth_flow() {
        let tester = StateMachineTester::new(AuthorizingStateMachine, FxaEvent::CancelOAuthFlow);
        assert_eq!(tester.state, Complete(FxaState::Connected));
    }

    /// Test restarting the scope authorization flow when one is already in progress.
    #[test]
    fn test_begin_oauth_scope_authorization_flow() {
        let tester = StateMachineTester::new(
            AuthorizingStateMachine,
            FxaEvent::BeginOAuthScopeAuthorizationFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            },
        );
        assert_eq!(
            tester.state,
            BeginOAuthScopeAuthorizationFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            }
        );
        assert_eq!(tester.peek_next_state(CallError), Cancel);
        assert_eq!(
            tester.peek_next_state(BeginOAuthFlowSuccess {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            }),
            Complete(FxaState::Authorizing {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            })
        );
    }

    #[test]
    fn test_disconnect_during_oauth_flow() {
        let tester = StateMachineTester::new(AuthorizingStateMachine, FxaEvent::Disconnect);
        assert_eq!(
            tester.peek_next_state(CallError),
            Complete(FxaState::Disconnected)
        );
        assert_eq!(
            tester.peek_next_state(DisconnectSuccess),
            Complete(FxaState::Disconnected)
        );
    }
}
