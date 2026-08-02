op asterisk-ari-asterisk-ping -> Any
  description "Response pong message."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/ping")
  response = http.request(method: "GET", url)
  return response
