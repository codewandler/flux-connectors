op slack-reactions-add(channel: String, timestamp: String, name: String) -> Any
  description "Add an emoji reaction to a message, visible to everyone in the channel. Slack answers HTTP 200 even on failure: check `ok` in the response body, where an error such as `already_reacted` appears at `error`. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://slack.com"
  url = fmt("{base}/api/reactions.add")
  content_type = "application/json"
  payload = { channel: $channel, name, timestamp }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
