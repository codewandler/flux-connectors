op nightly-sweep(tick_at: Any) -> Any
  description "Sweep yesterday's things, note them, and delete them under approval."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $window_query = fmt("updated_after:{tick_at}")
  retry 3 backoff exponential delay 250 -> $read_result
    $fetch_result = vendor-thing-search({ q: $window_query })
    $fetch_result
  throttle "nightly-sweep#paced" 5 per 60000
    $note_noted = vendor-thing-note({ body: $read_result })
  confirm "Delete the swept thing?" risk destructive
    $wipe_gone = vendor-thing-delete({ id: $read_result })
  when $read_result
    do vendor-thing-note { body: $note_noted }
  return $wipe_gone
