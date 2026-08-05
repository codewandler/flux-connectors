op stripe-customer-get(customer: String) -> Any
  description "Get one customer by id: name, email, phone, billing address, default payment method and account balance. This is personal data about a named individual — read it only when the task needs it, and do not repeat it further than the task requires. A deleted customer is returned with `deleted: true` and almost no other fields. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/customers/{customer}")
  response = http.request(method: "GET", url)
  return response
