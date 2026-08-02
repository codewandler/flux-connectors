op asterisk-ari-sounds-list(lang: String, format: String) -> Any
  description "List all sounds."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/sounds")
  sep = "?"
  when lang
    url = fmt("{url}{sep}lang={lang}")
    sep = "&"
  when format
    url = fmt("{url}{sep}format={format}")
  response = http.request(method: "GET", url)
  return response
