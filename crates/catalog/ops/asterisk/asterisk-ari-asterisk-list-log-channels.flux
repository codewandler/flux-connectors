op asterisk-ari-asterisk-list-log-channels -> Any
  description "Gets Asterisk log channel information."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/logging")
  response = http.request(method: "GET", url)
  return response
