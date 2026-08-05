op asterisk-ari-channels-unhold(channelId: String) -> Any
  description "Remove a channel from hold."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/hold")
  response = http.request(method: "DELETE", url)
  return response
