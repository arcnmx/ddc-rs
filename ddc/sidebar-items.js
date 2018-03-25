initSidebarItems({"constant":[["DELAY_COMMAND_FAILED_MS","DDC delay required before retrying a request"],["I2C_ADDRESS_DDC_CI","DDC/CI command and control I2C address"],["I2C_ADDRESS_EDID","EDID EEPROM I2C address"],["I2C_ADDRESS_EDID_SEGMENT","E-DDC EDID segment register I2C address"],["SUB_ADDRESS_DDC_CI","DDC sub-address command prefix"]],"enum":[["ErrorCode","DDC/CI protocol errors"],["VcpValueType","VCP feature type."]],"mod":[["commands","DDC/CI command request and response types."]],"struct":[["Delay","A type that can help with implementing the DDC specificationed delays."],["VcpValue","VCP Value"]],"trait":[["Ddc","A high level interface to DDC commands."],["DdcCommand","A (slightly) higher level interface to `DdcCommandRaw`."],["DdcCommandMarker","Using this marker trait will automatically implement the `Ddc` and `DdcTable` traits."],["DdcCommandRaw","Allows the execution of arbitrary low level DDC commands."],["DdcCommandRawMarker","Using this marker trait will automatically implement the `DdcCommand` trait."],["DdcHost","A DDC host is able to communicate with a DDC device such as a display."],["DdcTable","Table commands can read and write arbitrary binary data to a VCP feature."],["Eddc","E-DDC allows reading extensions of Enhanced EDID."],["Edid","A trait that allows retrieving Extended Display Identification Data (EDID) from a device."]],"type":[["FeatureCode","VCP feature code"]]});