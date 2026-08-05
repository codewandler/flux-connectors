op resend-email-get(email_id: String) -> Any
  description "Read one sent message and its current delivery state. This is how a flow learns whether a send bounced, was delivered, or is still queued"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.resend.com"
  url = fmt("{base}/emails/{email_id}")
  response = http.request(method: "GET", url)
  return response
