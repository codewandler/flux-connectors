op resend-email-send(from: String, to: Any, subject: String, html: String) -> Any
  description "Send one email immediately. Delivered within seconds and cannot be recalled once accepted. The sending domain must already be verified on this account — resend-domain-list names the ones that are"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.resend.com"
  url = fmt("{base}/emails")
  content_type = "application/json"
  payload = { from, html, subject, to }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
