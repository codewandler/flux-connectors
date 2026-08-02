op asterisk-ari-sounds-get(soundId: String) -> Any
  description "Get a sound's details."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/sounds/{soundId}")
  response = http.request(method: "GET", url)
  return response
