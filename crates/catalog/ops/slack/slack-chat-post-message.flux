op slack-chat-post-message(channel: String, text: String, thread_ts: String) -> Any
  description "Post a message to a Slack channel, visible to everyone in it. Slack answers HTTP 200 even on failure: check `ok` in the response body before treating the message as sent. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://slack.com"
  url = fmt("{base}/api/chat.postMessage")
  content_type = "application/json"
  payload = { channel: $channel, text, thread_ts }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
