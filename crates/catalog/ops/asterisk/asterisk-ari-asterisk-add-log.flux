op asterisk-ari-asterisk-add-log(logChannelName: String, configuration: String) -> Any
  description "Adds a log channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/logging/{logChannelName}")
  response = http.request(method: "POST", query: { configuration }, url)
  return response
