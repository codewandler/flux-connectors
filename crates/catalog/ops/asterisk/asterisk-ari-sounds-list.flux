op asterisk-ari-sounds-list(lang: String, format: String) -> Any
  description "List all sounds."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/sounds")
  response = http.request(method: "GET", query: { format, lang }, url)
  return response
