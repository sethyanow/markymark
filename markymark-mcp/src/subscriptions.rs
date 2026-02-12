//! Resource subscription tracking for MCP `resources/subscribe` protocol.
//!
//! Tracks which resource URIs clients have subscribed to and provides
//! notification dispatch when indexed content changes.

use std::collections::HashSet;
use std::sync::Mutex;

use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::service::Peer;
use rmcp::RoleServer;

/// Tracks resource subscriptions and sends `notifications/resources/updated`
/// when subscribed resources change.
pub struct SubscriptionTracker {
    /// Set of subscribed resource URIs.
    subscribed: Mutex<HashSet<String>>,
    /// Peer handle for sending notifications (set lazily on first subscribe).
    peer: Mutex<Option<Peer<RoleServer>>>,
}

impl SubscriptionTracker {
    /// Create an empty tracker with no subscriptions.
    pub fn new() -> Self {
        Self {
            subscribed: Mutex::new(HashSet::new()),
            peer: Mutex::new(None),
        }
    }

    /// Record a subscription and store the peer handle for future notifications.
    pub fn subscribe(&self, uri: String, peer: Peer<RoleServer>) {
        {
            let mut subs = self.subscribed.lock().expect("lock poisoned");
            subs.insert(uri);
        }
        {
            let mut p = self.peer.lock().expect("lock poisoned");
            *p = Some(peer);
        }
    }

    /// Record a subscription without a peer handle (for testing).
    pub fn track(&self, uri: String) {
        let mut subs = self.subscribed.lock().expect("lock poisoned");
        subs.insert(uri);
    }

    /// Remove a subscription. Returns `true` if the URI was previously subscribed.
    pub fn untrack(&self, uri: &str) -> bool {
        let mut subs = self.subscribed.lock().expect("lock poisoned");
        subs.remove(uri)
    }

    /// Check whether a URI is currently subscribed.
    pub fn is_subscribed(&self, uri: &str) -> bool {
        let subs = self.subscribed.lock().expect("lock poisoned");
        subs.contains(uri)
    }

    /// Return the count of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        let subs = self.subscribed.lock().expect("lock poisoned");
        subs.len()
    }

    /// Notify ALL subscribed URIs that their content may have changed.
    ///
    /// Called after realm mutations (add-root, remove-root, create/destroy realm)
    /// to inform clients that previously-read resources may have new data.
    pub async fn notify_all(&self) {
        let uris: Vec<String> = {
            let subs = self.subscribed.lock().expect("lock poisoned");
            subs.iter().cloned().collect()
        };

        if uris.is_empty() {
            return;
        }

        let peer = {
            let p = self.peer.lock().expect("lock poisoned");
            p.clone()
        };

        if let Some(peer) = peer {
            for uri in uris {
                let _ = peer
                    .notify_resource_updated(ResourceUpdatedNotificationParam { uri })
                    .await;
            }
        }
    }
}

impl Default for SubscriptionTracker {
    fn default() -> Self {
        Self::new()
    }
}
