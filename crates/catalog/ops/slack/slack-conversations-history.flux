op slack-conversations-history(channel: String, limit: Number, oldest: String, latest: String) -> Any
  description "Read recent messages from a Slack channel, newest first. Slack answers HTTP 200 even on failure: check `ok` in the response body, where an error such as `channel_not_found` appears at `error`. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://slack.com"
  url = fmt("{base}/api/conversations.history")
  content_type = "application/json"
  payload = { channel: $channel, latest, limit, oldest }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
