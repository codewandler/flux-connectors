op asterisk-ari-channels-answer(channelId: String) -> Any
  description "Answer a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/answer")
  response = http.request(method: "POST", url)
  return response
