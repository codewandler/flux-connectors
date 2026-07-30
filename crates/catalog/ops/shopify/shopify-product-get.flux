op shopify-product-get(product_id: Number) -> Any
  description "Get one product by id, with its title, description, status, vendor, options, variants and images. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{shop}.myshopify.com"
  url = fmt("{base}/admin/api/2024-10/products/{product_id}.json")
  response = http.request(method: "GET", url)
  return response
