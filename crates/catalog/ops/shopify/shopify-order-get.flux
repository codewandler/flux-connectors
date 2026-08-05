op shopify-order-get(order_id: Number) -> Any
  description "Get one order by id, with its line items, totals, fulfilment and financial status, and the customer and addresses attached to it. The response carries personal data: the customer's name, email, phone and shipping address. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{shop}.myshopify.com"
  url = fmt("{base}/admin/api/2024-10/orders/{order_id}.json")
  response = http.request(method: "GET", url)
  return response
