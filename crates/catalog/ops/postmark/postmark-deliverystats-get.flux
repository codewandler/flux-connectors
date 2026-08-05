op postmark-deliverystats-get -> Any
  description "Get delivery statistics for this server: total inactive addresses and a breakdown of bounces by type. Also this connector's `verify` — a bounded read that runs unattended, needing no argument. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/Message`, its error code at `/ErrorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.postmarkapp.com"
  url = fmt("{base}/deliverystats")
  response = http.request(method: "GET", url)
  return response
