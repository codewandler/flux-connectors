op shopify-inventory-level-list(location_id: Number) -> Any
  description "List the inventory levels held at one location — the available quantity per inventory item. Returns the first page only, so a location stocking more items than one page holds is reported incompletely. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{shop}.myshopify.com"
  url = fmt("{base}/admin/api/2024-10/locations/{location_id}/inventory_levels.json")
  response = http.request(method: "GET", url)
  return response
