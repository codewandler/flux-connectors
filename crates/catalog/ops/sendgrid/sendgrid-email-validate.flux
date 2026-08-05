op sendgrid-email-validate(email: String, source: String) -> Any
  description "Check whether an email address is well-formed and likely deliverable, without sending it any mail. Requires SendGrid's Email Address Validation add-on to be enabled on the account. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/validations/email")
  content_type = "application/json"
  payload = { email, source }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
