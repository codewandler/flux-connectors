op asterisk-ari-asterisk-update-object(configClass: String, objectType: String, id: String, body: Any) -> Any
  description "Create or update a dynamic configuration object."
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/config/dynamic/{configClass}/{objectType}/{id}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
