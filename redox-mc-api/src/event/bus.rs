//! Strongly-typed thread-safe pub/sub event bus with priority and cancellation.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Priority levels for event handlers.
/// Handlers are executed strictly from Highest to Lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum EventPriority {
    /// Dispatched first. Used for high-priority filters, security checks, and early logic.
    Highest = 0,
    /// Default priority for standard game and mod event handlers.
    #[default]
    Normal = 1,
    /// Dispatched last. Used for monitoring, telemetry, and final reactions.
    Lowest = 2,
}

/// The result returned by an event dispatch or event handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventResult {
    /// Event processing continues normally.
    Allow,
    /// Event is cancelled; halts propagation to lower-priority handlers.
    Cancel,
}

impl EventResult {
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancel)
    }

    #[inline]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// Conversion trait allowing event handlers to return either `()`, `bool`, or `EventResult`.
pub trait IntoEventResult {
    fn into_event_result(self) -> EventResult;
}

impl IntoEventResult for EventResult {
    #[inline]
    fn into_event_result(self) -> EventResult {
        self
    }
}

impl IntoEventResult for () {
    #[inline]
    fn into_event_result(self) -> EventResult {
        EventResult::Allow
    }
}

impl IntoEventResult for bool {
    #[inline]
    fn into_event_result(self) -> EventResult {
        if self {
            EventResult::Allow
        } else {
            EventResult::Cancel
        }
    }
}

/// Base trait for all events publishable through `EventBus`.
pub trait Event: 'static {
    /// Returns true if this event is currently cancelled.
    /// Default implementation always returns `false`.
    fn is_cancelled(&self) -> bool {
        false
    }

    /// Sets the cancelled state of this event.
    /// Default implementation is a no-op for non-cancellable events.
    fn set_cancelled(&mut self, _cancelled: bool) {}
}

/// Marker and helper trait for events that support explicit cancellation.
pub trait CancellableEvent: Event {
    /// Convenience helper to cancel the event.
    #[inline]
    fn cancel(&mut self) {
        self.set_cancelled(true);
    }
}

/// Unique identifier for an active event subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubscriptionId(u64);

type ErasedHandler = Box<dyn FnMut(&mut (dyn Any + 'static)) -> EventResult + Send>;

struct HandlerEntry {
    id: SubscriptionId,
    priority: EventPriority,
    order: u64,
    handler: ErasedHandler,
}

/// A thread-safe, strongly-typed publish/subscribe event bus.
pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<Arc<Mutex<HandlerEntry>>>>>,
    next_sub_id: AtomicU64,
    registration_counter: AtomicU64,
}

impl EventBus {
    /// Creates a new, empty `EventBus`.
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            next_sub_id: AtomicU64::new(1),
            registration_counter: AtomicU64::new(1),
        }
    }

    /// Subscribes a typed handler with default priority (`EventPriority::Normal`).
    pub fn subscribe<E, F, R>(&self, handler: F) -> SubscriptionId
    where
        E: Event,
        F: FnMut(&mut E) -> R + Send + 'static,
        R: IntoEventResult,
    {
        self.subscribe_with_priority(EventPriority::Normal, handler)
    }

    /// Subscribes a typed handler with an explicit priority.
    pub fn subscribe_with_priority<E, F, R>(
        &self,
        priority: EventPriority,
        mut handler: F,
    ) -> SubscriptionId
    where
        E: Event,
        F: FnMut(&mut E) -> R + Send + 'static,
        R: IntoEventResult,
    {
        let id = SubscriptionId(self.next_sub_id.fetch_add(1, Ordering::Relaxed));
        let order = self.registration_counter.fetch_add(1, Ordering::Relaxed);

        let type_id = TypeId::of::<E>();
        let erased_handler = move |any_event: &mut (dyn Any + 'static)| -> EventResult {
            if let Some(event) = any_event.downcast_mut::<E>() {
                let res = handler(event).into_event_result();
                if res == EventResult::Cancel || event.is_cancelled() {
                    event.set_cancelled(true);
                    EventResult::Cancel
                } else {
                    EventResult::Allow
                }
            } else {
                EventResult::Allow
            }
        };

        let entry = Arc::new(Mutex::new(HandlerEntry {
            id,
            priority,
            order,
            handler: Box::new(erased_handler),
        }));

        let mut map = self.handlers.write().unwrap();
        map.entry(type_id).or_default().push(entry);

        id
    }

    /// Unsubscribes a previously registered handler by its `SubscriptionId`.
    ///
    /// Returns `true` if a handler was found and removed, `false` otherwise.
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut map = self.handlers.write().unwrap();
        let mut found = false;
        for entries in map.values_mut() {
            let initial_len = entries.len();
            entries.retain(|e| e.lock().unwrap().id != id);
            if entries.len() < initial_len {
                found = true;
            }
        }
        found
    }

    /// Dispatches an event to all registered handlers in strict priority order.
    ///
    /// If any handler cancels the event (via return value or `CancellableEvent::cancel`),
    /// propagation halts immediately and subsequent lower-priority handlers are NOT called.
    pub fn dispatch<E: Event>(&self, event: &mut E) -> EventResult {
        let type_id = TypeId::of::<E>();

        // Copy handler references under read lock, then execute outside the read lock
        let entries_to_run = {
            let map = self.handlers.read().unwrap();
            map.get(&type_id).cloned()
        };

        let Some(mut entries) = entries_to_run else {
            return EventResult::Allow;
        };

        // Sort entries by (priority ascending, registration order ascending)
        entries.sort_by_key(|e| {
            let guard = e.lock().unwrap();
            (guard.priority, guard.order)
        });

        // Initial check in case event was already cancelled prior to dispatch
        if event.is_cancelled() {
            return EventResult::Cancel;
        }

        for entry in entries {
            let mut guard = entry.lock().unwrap();
            let res = (guard.handler)(event as &mut (dyn Any + 'static));

            if res == EventResult::Cancel || event.is_cancelled() {
                event.set_cancelled(true);
                return EventResult::Cancel;
            }
        }

        if event.is_cancelled() {
            EventResult::Cancel
        } else {
            EventResult::Allow
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_BUS: OnceLock<EventBus> = OnceLock::new();

/// Returns a reference to the global default `EventBus`.
pub fn global() -> &'static EventBus {
    GLOBAL_BUS.get_or_init(EventBus::new)
}
