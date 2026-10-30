import json,pathlib,statistics,math,sys
root=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else 'artifacts/analysis-million/formal-r3')
def read(name):return json.loads((root/(name+'.json')).read_text(encoding='utf-8-sig'))
def stats(v):
 v=sorted(v)
 return {'min':v[0],'median':statistics.median(v),'p95':v[math.ceil(.95*len(v))-1],'max':v[-1]}
summary={'comparisons':{},'scale':{},'acceptance':{}}
matched=read('environment').get('matched_current_baseline',False)
summary['comparison_policy']='no more than 10% regression' if matched else 'historical 30% improvement target'
for count in ['100000','500000']:
 old=[read(f'baseline-{count}-{r}') for r in range(1,6)]
 new=[read(f'current-{count}-{r}') for r in range(1,6)]
 for a,b in zip(old,new):
  for field in ['entries','files','logical_bytes','unique_bytes','state']:
   assert a[field]==b[field],(count,field,a[field],b[field])
 gain=1-statistics.median(x['elapsed_ms'] for x in new)/statistics.median(x['elapsed_ms'] for x in old)
 summary['comparisons'][count]={'baseline_ms':stats([x['elapsed_ms'] for x in old]),'current_ms':stats([x['elapsed_ms'] for x in new]),'baseline_native_peak_bytes':stats([x['peak_private_bytes'] for x in old]),'current_native_peak_bytes':stats([x['peak_private_bytes'] for x in new]),'median_reduction':gain,'counts':{k:new[0][k] for k in ['entries','files','logical_bytes','unique_bytes','state']}}
 summary['acceptance']['speed_'+count]=gain>=(-.10 if matched else .30)
for count in ['1000000','3000000']:
 rows=[read(f'current-{count}-{r}') for r in range(1,4)]
 for r in rows:
  assert r['state']=='completed' and r['files']==int(count)
  assert r['entries']=={'1000000':1001001,'3000000':3003003}[count]
  assert int(r['logical_bytes'])==int(r['unique_bytes'])==int(count)*7
  assert int(r['allocated_bytes'])==int(count)*8, 'fixture allocation must match the measured NTFS oracle'
  assert r['allocation_unknown']==0
 ordinary=[q['elapsed_us']/1000 for r in rows for q in r['read_queries'] if (q['search']=='' or q['view']=='discovery') and q['ok']]
 search=[q['elapsed_us']/1000 for r in rows for q in r['read_queries'] if q['view']=='files' and q['search']!='' and q['ok']]
 summary['scale'][count]={'elapsed_ms':stats([r['elapsed_ms'] for r in rows]),'first_entries_ms':stats([r['first_entries_ms'] for r in rows]),'native_peak_bytes':max(r['peak_private_bytes'] for r in rows),'index_peak_sampled_bytes':max(r['peak_index_disk_bytes'] for r in rows),'page_ms':stats(ordinary),'name_search_ms':stats(search),'cancel_read_ms':max(r['cancel_read_ms'] for r in rows),'query_errors':[q for r in rows for q in r['read_queries'] if not q['ok']]}
 summary['acceptance']['memory_'+count]=summary['scale'][count]['native_peak_bytes']<=900*1024**2
 summary['acceptance']['index_'+count]=summary['scale'][count]['index_peak_sampled_bytes']<4*1024**3
 summary['acceptance']['ordinary_page_'+count]=max(ordinary)<=250
 summary['acceptance']['name_search_'+count]=max(search)<=2000
 summary['acceptance']['query_success_'+count]=all(q['ok'] for r in rows for q in r['read_queries'])
 summary['acceptance']['query_cancel_'+count]=all(r['cancel_read_interrupted'] and r['cancel_read_ms']<=1000 for r in rows)
 summary['scale'][count]['cold_first_page_ms']=stats([r['cold_query']['elapsed_us']/1000 for r in rows])
 summary['acceptance']['cold_query_success_'+count]=all(r['cold_query']['ok'] for r in rows)
 if matched:
  summary['acceptance']['cold_first_page_'+count]=all(r['cold_query']['ok'] and r['cold_query']['elapsed_us']<=250000 for r in rows)
 if all('directory_query_plans' in r for r in rows):
  summary['acceptance']['directory_covering_index_'+count]=all(any('COVERING INDEX directory_ranking' in p for p in r['directory_query_plans']) for r in rows)
summary['cancellations']={name:read(name) for name in ['cancel-scan','cancel-query-during-scan','cancel-index']}
for name,row in summary['cancellations'].items():
 summary['acceptance'][name]=row['state']=='cancelled' and row['cancel_ms']<=1000
probe=summary['cancellations']['cancel-query-during-scan']['during_query']
summary['acceptance']['query_cancel_keeps_scan_active']=probe is not None and not probe['ok'] and probe['scan_still_active'] and probe['cancel_read_ms']<=1000
(root/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
print(json.dumps({k:v for k,v in summary.items() if k!='cancellations'},ensure_ascii=False,indent=2))
if not all(summary['acceptance'].values()):
 raise SystemExit(1)
