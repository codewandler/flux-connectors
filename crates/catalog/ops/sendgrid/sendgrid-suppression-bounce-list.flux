op sendgrid-suppression-bounce-list(start_time: Number, end_time: Number) -> Any
  description "List addresses that have bounced and are suppressed from receiving further mail. Each entry is personal data about a third party — read it only for what the calling flow needs"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/suppression/bounces")
  response = http.request(method: "GET", query: { end_time, start_time }, url)
  return response
