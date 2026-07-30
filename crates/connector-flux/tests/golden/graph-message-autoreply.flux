op message-autoreply(wake_event: Any) -> Any
  description "Reply in-thread when the app is mentioned."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  thread_out = wake_event.event.thread_ts
  channel_out = wake_event.event.channel
  greeting_out = fmt("Thanks for the ping in {thread_out}")
  when thread_out
    vendor-message-post(channel: channel_out, text: greeting_out)
