op asterisk-ari-endpoints-send-message(to: String, from: String, body: String, body_2: Any) -> Any
  description "Send a message to some technology URI or endpoint."
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/endpoints/sendMessage?to={to}&from={from}")
  sep = "&"
  when body
    url = fmt("{url}{sep}body={body}")
  content_type = "application/json"
  payload = parse(body_2, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
