op asterisk-ari-asterisk-delete-log(logChannelName: String) -> Any
  description "Deletes a log channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/logging/{logChannelName}")
  response = http.request(method: "DELETE", url)
  return response
