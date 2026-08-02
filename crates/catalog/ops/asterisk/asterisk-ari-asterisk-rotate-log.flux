op asterisk-ari-asterisk-rotate-log(logChannelName: String) -> Any
  description "Rotates a log channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/logging/{logChannelName}/rotate")
  response = http.request(method: "PUT", url)
  return response
