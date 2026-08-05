op asterisk-ari-channels-get(channelId: String) -> Any
  description "Channel details."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}")
  response = http.request(method: "GET", url)
  return response
