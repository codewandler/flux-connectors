op asterisk-ari-channels-stop-silence(channelId: String) -> Any
  description "Stop playing silence to a channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/silence")
  response = http.request(method: "DELETE", url)
  return response
