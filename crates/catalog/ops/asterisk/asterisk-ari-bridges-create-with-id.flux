op asterisk-ari-bridges-create-with-id(bridgeId: String, type: String, name: String, body: Any) -> Any
  description "Create a new bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}")
  sep = "?"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when name
    url = fmt("{url}{sep}name={name}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
