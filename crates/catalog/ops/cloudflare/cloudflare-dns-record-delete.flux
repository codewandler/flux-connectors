op cloudflare-dns-record-delete(dns_record_id: String) -> Any
  description "Delete one DNS record. There is no API route back: Cloudflare's dashboard \"recently deleted\" surface is a retention-window UI feature, not an endpoint, so a flux run cannot undo this. Responds with just the deleted record's id. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.cloudflare.com/client/v4"
  zone_id = "{zone_id}"
  url = fmt("{base}/zones/{zone_id}/dns_records/{dns_record_id}")
  response = http.request(method: "DELETE", url)
  return response
