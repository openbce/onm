# ufmclient
The rust client of UFM

## Example
### Set environment
```
export UFM_ADDRESS=$ufm_server_address
export UFM_TOKEN=$ufm_server_token
```
### Using user token Authentication
```
env UFM_TOKEN=XlojlA7zgotVegyIEIP5vnw5C7ZYT9 UFM_ADDRESS=https://ufm ./ufmctl version
6.11.1-2
```
### Using Client Authentication
```
env UFM_CA_CRT=ca.crt UFM_TLS_CRT=client.crt UFM_TLS_KEY=client.key UFM_ADDRESS=https://ufm ./ufmctl version
6.11.1-2
```
### Version
```
./ufmctl version
6.11.1-2
```
### Create a Partition Key
```
./ufmctl create --pkey 5 --mtu 2 --membership full --service-level 0 --rate-limit 2.5 --guids 0011223344560200 --guids 1070fd0300176625 --guids 0011223344560201
```

### View a Partition Key
```
./ufmctl view --pkey 0x5
Name           : api_pkey_0x5
Pkey           : 0x5
IPoIB          : false
MTU            : 2
Rate Limit     : 2.5
Service Level  : 0
Ports          : 
    GUID                ParentGUID          PortType  SystemID            LID       SystemName     LogState  Name                
    0011223344560200    1070fd0300176624    virtual   1070fd0300176624    7         hpc-cloud01    Active                        
    1070fd0300176625                        physical  1070fd0300176624    4         hpc-cloud01    Active    1070fd0300176625_2  
    0011223344560201                                                      65535                    Unknown  

```

### List Partition Keys
```
./ufmctl list
Name           Pkey      IPoIB     MTU       Rate      Level     
api_pkey_0x5   0x5       false     2         2.5       0         
api_pkey_0x2   0x2       false     2         2.5       0         
management     0x7fff    true      2         2.5       0         
api_pkey_0x1   0x1       false     2         2.5       0         
api_pkey_0x4   0x4       false     2         2.5       0  
```

### Delete a Partition Key
```
./ufmctl delete --pkey 0x2
```