op asterisk-ari-channels-dial(channelId: String, caller: String, timeout: Number) -> Any
  description "Dial a created channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/dial")
  response = http.request(method: "POST", query: { caller, timeout: $timeout }, url)
  return response
