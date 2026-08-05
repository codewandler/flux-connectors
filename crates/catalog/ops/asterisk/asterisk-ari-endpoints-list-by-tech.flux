op asterisk-ari-endpoints-list-by-tech(tech: String) -> Any
  description "List available endoints for a given endpoint technology."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/{tech}")
  response = http.request(method: "GET", url)
  return response
