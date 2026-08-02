op asterisk-ari-bridges-create(type: String, bridgeId: String, name: String, body: Any) -> Any
  description "Create a new bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges")
  sep = "?"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when bridgeId
    url = fmt("{url}{sep}bridgeId={bridgeId}")
    sep = "&"
  when name
    url = fmt("{url}{sep}name={name}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
