op asterisk-ari-endpoints-refer-to-endpoint(tech: String, resource: String, from: String, refer_to: String, to_self: Bool, body: Any) -> Any
  description "Refer an endpoint or technology URI to some technology URI or endpoint."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/{tech}/{resource}/refer?from={from}&refer_to={refer_to}")
  sep = "&"
  when to_self
    url = fmt("{url}{sep}to_self={to_self}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
