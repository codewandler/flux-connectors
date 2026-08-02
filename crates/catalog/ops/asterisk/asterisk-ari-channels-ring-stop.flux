op asterisk-ari-channels-ring-stop(channelId: String) -> Any
  description "Stop ringing indication on a channel if locally generated."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/ring")
  response = http.request(method: "DELETE", url)
  return response
