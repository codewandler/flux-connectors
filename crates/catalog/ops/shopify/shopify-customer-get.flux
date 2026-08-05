op shopify-customer-get(customer_id: Number) -> Any
  description "Get one customer by id: name, email, phone, default address, marketing consent state and order count. This is personal data about a named individual — read it only when the task needs it, and do not repeat it further than the task requires. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{shop}.myshopify.com"
  url = fmt("{base}/admin/api/2024-10/customers/{customer_id}.json")
  response = http.request(method: "GET", url)
  return response
