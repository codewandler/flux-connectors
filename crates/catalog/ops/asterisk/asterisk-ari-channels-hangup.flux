op asterisk-ari-channels-hangup(channelId: String, reason_code: String, reason: String) -> Any
  description "Delete (i.e. hangup) a channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}")
  response = http.request(method: "DELETE", query: { reason, reason_code }, url)
  return response
