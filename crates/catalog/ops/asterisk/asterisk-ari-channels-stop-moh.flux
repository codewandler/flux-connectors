op asterisk-ari-channels-stop-moh(channelId: String) -> Any
  description "Stop playing music on hold to a channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/moh")
  response = http.request(method: "DELETE", url)
  return response
