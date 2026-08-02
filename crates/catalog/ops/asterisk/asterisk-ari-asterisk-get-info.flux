op asterisk-ari-asterisk-get-info(only: List<String>) -> Any
  description "Gets Asterisk system information."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/info")
  sep = "?"
  when only
    url = fmt("{url}{sep}only={only}")
  response = http.request(method: "GET", url)
  return response
