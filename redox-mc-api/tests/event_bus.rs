use redox_mc_api::event::{CancellableEvent, Event, EventBus, EventPriority, EventResult};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct TestMessageEvent {
    #[allow(dead_code)]
    message: String,
    visits: Vec<String>,
}

impl Event for TestMessageEvent {}

#[derive(Debug)]
struct TestCancelEvent {
    #[allow(dead_code)]
    id: u32,
    cancelled: bool,
}

impl Event for TestCancelEvent {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn set_cancelled(&mut self, cancelled: bool) {
        self.cancelled = cancelled;
    }
}

impl CancellableEvent for TestCancelEvent {}

#[test]
fn test_event_bus_basic_dispatch() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    bus.subscribe::<TestMessageEvent, _, _>(move |event| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
        event.visits.push("handler_1".into());
    });

    let mut event = TestMessageEvent {
        message: "hello".into(),
        visits: Vec::new(),
    };

    let result = bus.dispatch(&mut event);
    assert_eq!(result, EventResult::Allow);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(event.visits, vec!["handler_1"]);
}

#[test]
fn test_event_priority_ordering() {
    let bus = EventBus::new();
    let order = Arc::new(Mutex::new(Vec::new()));

    // Register in reverse order: Lowest, Normal, Highest
    let o1 = order.clone();
    bus.subscribe_with_priority::<TestMessageEvent, _, _>(EventPriority::Lowest, move |_| {
        o1.lock().unwrap().push("lowest");
    });

    let o2 = order.clone();
    bus.subscribe_with_priority::<TestMessageEvent, _, _>(EventPriority::Normal, move |_| {
        o2.lock().unwrap().push("normal");
    });

    let o3 = order.clone();
    bus.subscribe_with_priority::<TestMessageEvent, _, _>(EventPriority::Highest, move |_| {
        o3.lock().unwrap().push("highest");
    });

    let mut event = TestMessageEvent {
        message: "priority test".into(),
        visits: Vec::new(),
    };

    let result = bus.dispatch(&mut event);
    assert_eq!(result, EventResult::Allow);

    let recorded = order.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec!["highest", "normal", "lowest"],
        "Handlers must be executed strictly in order: Highest -> Normal -> Lowest"
    );
}

#[test]
fn test_cancellation_stops_propagation() {
    let bus = EventBus::new();
    let normal_called = Arc::new(AtomicBool::new(false));
    let lowest_called = Arc::new(AtomicBool::new(false));

    // Highest handler cancels the event
    bus.subscribe_with_priority::<TestCancelEvent, _, _>(EventPriority::Highest, |_| {
        EventResult::Cancel
    });

    let n_called = normal_called.clone();
    bus.subscribe_with_priority::<TestCancelEvent, _, _>(EventPriority::Normal, move |_| {
        n_called.store(true, Ordering::SeqCst);
    });

    let l_called = lowest_called.clone();
    bus.subscribe_with_priority::<TestCancelEvent, _, _>(EventPriority::Lowest, move |_| {
        l_called.store(true, Ordering::SeqCst);
    });

    let mut event = TestCancelEvent {
        id: 42,
        cancelled: false,
    };

    let result = bus.dispatch(&mut event);
    assert_eq!(result, EventResult::Cancel);
    assert!(
        event.is_cancelled(),
        "Event must be marked as cancelled when handler returns Cancel"
    );
    assert!(
        !normal_called.load(Ordering::SeqCst),
        "Normal priority handler must not be called after cancellation"
    );
    assert!(
        !lowest_called.load(Ordering::SeqCst),
        "Lowest priority handler must not be called after cancellation"
    );
}

#[test]
fn test_cancellable_trait_method_cancel() {
    let bus = EventBus::new();
    let second_called = Arc::new(AtomicBool::new(false));
    let s_clone = second_called.clone();

    bus.subscribe_with_priority::<TestCancelEvent, _, _>(EventPriority::Highest, |event| {
        event.cancel(); // Using CancellableEvent::cancel(&mut self)
    });

    bus.subscribe_with_priority::<TestCancelEvent, _, _>(EventPriority::Normal, move |_| {
        s_clone.store(true, Ordering::SeqCst);
    });

    let mut event = TestCancelEvent {
        id: 100,
        cancelled: false,
    };

    let result = bus.dispatch(&mut event);
    assert_eq!(result, EventResult::Cancel);
    assert!(event.is_cancelled());
    assert!(!second_called.load(Ordering::SeqCst));
}

#[test]
fn test_unsubscribe() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let sub_id = bus.subscribe::<TestMessageEvent, _, _>(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    let mut event = TestMessageEvent {
        message: "test".into(),
        visits: Vec::new(),
    };

    bus.dispatch(&mut event);
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Unsubscribe
    assert!(bus.unsubscribe(sub_id));

    bus.dispatch(&mut event);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Handler must not be invoked after being unsubscribed"
    );
}

#[test]
fn test_multiple_event_types_isolation() {
    let bus = EventBus::new();
    let msg_count = Arc::new(AtomicUsize::new(0));
    let cancel_count = Arc::new(AtomicUsize::new(0));

    let m = msg_count.clone();
    bus.subscribe::<TestMessageEvent, _, _>(move |_| {
        m.fetch_add(1, Ordering::SeqCst);
    });

    let c = cancel_count.clone();
    bus.subscribe::<TestCancelEvent, _, _>(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    let mut msg_event = TestMessageEvent {
        message: "a".into(),
        visits: Vec::new(),
    };
    bus.dispatch(&mut msg_event);

    assert_eq!(msg_count.load(Ordering::SeqCst), 1);
    assert_eq!(cancel_count.load(Ordering::SeqCst), 0);

    let mut cancel_event = TestCancelEvent {
        id: 1,
        cancelled: false,
    };
    bus.dispatch(&mut cancel_event);

    assert_eq!(msg_count.load(Ordering::SeqCst), 1);
    assert_eq!(cancel_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_concurrent_dispatch_thread_safety() {
    let bus = Arc::new(EventBus::new());
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    bus.subscribe::<TestMessageEvent, _, _>(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    let mut handles = Vec::new();
    for _ in 0..10 {
        let b = bus.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                let mut ev = TestMessageEvent {
                    message: "thread".into(),
                    visits: Vec::new(),
                };
                b.dispatch(&mut ev);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1000,
        "All 10 threads must complete 100 dispatches concurrently"
    );
}

#[test]
fn test_server_tick_event_dispatch() {
    use redox_mc_api::event::ServerTickEvent;

    let bus = EventBus::new();
    let tick_observed = Arc::new(AtomicUsize::new(0));
    let t_clone = tick_observed.clone();

    bus.subscribe::<ServerTickEvent, _, _>(move |event| {
        t_clone.store(event.tick_count as usize, Ordering::SeqCst);
    });

    let mut event = ServerTickEvent::new(42);
    let result = bus.dispatch(&mut event);

    assert_eq!(result, EventResult::Allow);
    assert_eq!(tick_observed.load(Ordering::SeqCst), 42);
}

#[test]
fn test_server_chat_event_cancellation() {
    use redox_mc_api::event::ServerChatEvent;

    let bus = EventBus::new();

    // Security/command filter: cancel messages starting with '!'
    bus.subscribe_with_priority::<ServerChatEvent, _, _>(EventPriority::Highest, |event| {
        if event.message().starts_with('!') {
            event.cancel();
        }
    });

    let normal_called = Arc::new(AtomicBool::new(false));
    let n_clone = normal_called.clone();
    bus.subscribe_with_priority::<ServerChatEvent, _, _>(EventPriority::Normal, move |_| {
        n_clone.store(true, Ordering::SeqCst);
    });

    // Test 1: Chat message with '!' should be cancelled and normal handler should not run
    let mut cmd_chat = ServerChatEvent::new("Alice", "!help");
    let result1 = bus.dispatch(&mut cmd_chat);
    assert_eq!(result1, EventResult::Cancel);
    assert!(cmd_chat.is_cancelled());
    assert!(!normal_called.load(Ordering::SeqCst));

    // Test 2: Standard chat message should pass through
    let mut normal_chat = ServerChatEvent::new("Bob", "Hello world");
    let result2 = bus.dispatch(&mut normal_chat);
    assert_eq!(result2, EventResult::Allow);
    assert!(!normal_chat.is_cancelled());
    assert!(normal_called.load(Ordering::SeqCst));
}
