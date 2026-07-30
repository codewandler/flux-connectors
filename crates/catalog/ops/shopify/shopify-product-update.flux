op shopify-product-update(product_id: Number, title: String) -> Any
  description "Rename a product. The new title is live on the public storefront as soon as this returns, on every sales channel the product is published to. Shopify applies only the fields sent, so nothing else about the product changes. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{shop}.myshopify.com"
  url = fmt("{base}/admin/api/2024-10/products/{product_id}.json")
  content_type = "application/json"
  payload = { product: { title } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
