op stripe-refund-get(refund: String) -> Any
  description "Get one refund by id: its amount, the charge it belongs to, its reason and its status. A refund that reports `pending` has not reached the customer's bank yet, and one that reports `failed` means the money came back — the customer was not paid. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/refunds/{refund}")
  response = http.request(method: "GET", url)
  return response
