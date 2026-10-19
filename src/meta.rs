use crate::{Result, xml_document, xml_escape};
use xmltree::{Element, XMLNode};
pub fn parse_element(text: &str) -> Result<Element> {
    xml_document(text)?;
    Element::parse(text.as_bytes()).map_err(|e| e.to_string())
}
pub fn write_element(root: &Element) -> Result<String> {
    let mut bytes = vec![];
    root.write_with_config(
        &mut bytes,
        xmltree::EmitterConfig::new()
            .perform_indent(true)
            .write_document_declaration(true),
    )
    .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
pub fn format_xml(text: &str) -> Result<String> {
    write_element(&parse_element(text)?)
}
pub fn set_value(root: &mut Element, name: &str, value: &str, attribute: bool) {
    if root.get_child(name).is_none() {
        root.children.push(XMLNode::Element(Element::new(name)));
    }
    let n = root.get_mut_child(name).unwrap();
    if attribute {
        n.attributes.insert("value".into(), value.into());
    } else {
        n.children
            .retain(|n| !matches!(n, XMLNode::Text(_) | XMLNode::CData(_)));
        n.children.push(XMLNode::Text(value.into()));
    }
}
pub fn schema(kind: usize) -> (&'static str, &'static str, &'static str) {
    match kind {
        0 => ("CHandlingDataMgr", "HandlingData", "handlingName"),
        1 => ("CVehicleModelInfo__InitDataList", "InitDatas", "modelName"),
        2 => ("CVehicleModelInfoVariation", "variationData", "modelName"),
        _ => ("CVehicleModelInfoVarGlobal", "Sirens", "name"),
    }
}
pub fn entries(text: &str, kind: usize) -> Result<Vec<String>> {
    let root = parse_element(text)?;
    let locations = entry_locations(&root, kind)?;
    Ok(locations
        .iter()
        .enumerate()
        .map(|(i, &(container, item))| {
            let section = root.children[container].as_element().unwrap();
            let item = section.children[item].as_element().unwrap();
            let name = item
                .get_child(schema(kind).2)
                .and_then(|n| n.get_text())
                .map(|v| v.to_string())
                .or_else(|| {
                    item.get_child("id")
                        .and_then(|n| n.attributes.get("value"))
                        .map(|v| format!("ID {v}"))
                })
                .unwrap_or_else(|| format!("Entry {}", i + 1));
            if kind == 3 {
                format!("{} / {name}", section.name)
            } else {
                name
            }
        })
        .collect())
}

/// Detect the document type from its XML root, regardless of the selected tab.
pub fn detect_kind(text: &str) -> Result<usize> {
    let root = parse_element(text)?;
    (0..4)
        .find(|&kind| root.name == schema(kind).0)
        .ok_or_else(|| format!("Unsupported metadata root: {}.", root.name))
}

fn entry_locations(root: &Element, kind: usize) -> Result<Vec<(usize, usize)>> {
    let (expected, container, _) = schema(kind);
    if root.name != expected {
        return Err(format!("Expected {expected}."));
    }
    let containers: &[&str] = if kind == 3 {
        &["Sirens", "Lights", "Kits"]
    } else {
        &[container]
    };
    let locations: Vec<_> = root
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, n)| n.as_element().map(|n| (i, n)))
        .filter(|(_, n)| containers.contains(&n.name.as_str()))
        .flat_map(|(i, n)| {
            n.children.iter().enumerate().filter_map(move |(j, n)| {
                n.as_element().filter(|n| n.name == "Item").map(|_| (i, j))
            })
        })
        .collect();
    if locations.is_empty() {
        return Err("No editable entries.".into());
    }
    Ok(locations)
}
#[derive(Clone)]
pub struct Field {
    pub path: String,
    pub value: String,
}
pub fn fields(text: &str, kind: usize, index: usize) -> Result<Vec<Field>> {
    let root = parse_element(text)?;
    let (container, item) = entry_locations(&root, kind)?
        .get(index)
        .copied()
        .ok_or("Missing selected entry")?;
    let item = root.children[container].as_element().unwrap().children[item]
        .as_element()
        .unwrap();
    let mut fields = vec![];
    for n in item.children.iter().filter_map(|n| n.as_element()) {
        if n.children.iter().any(|n| n.as_element().is_some()) {
            continue;
        }
        if n.attributes.is_empty() {
            fields.push(Field {
                path: n.name.clone(),
                value: n.get_text().map(|s| s.into_owned()).unwrap_or_default(),
            });
        } else {
            let mut attrs: Vec<_> = n.attributes.iter().collect();
            attrs.sort_by_key(|(k, _)| *k);
            for (k, v) in attrs {
                fields.push(Field {
                    path: format!("{}@{k}", n.name),
                    value: v.clone(),
                });
            }
        }
    }
    Ok(fields)
}
pub fn update(text: &str, kind: usize, index: usize, fields: &[Field]) -> Result<String> {
    let mut root = parse_element(text)?;
    let (container, item) = entry_locations(&root, kind)?
        .get(index)
        .copied()
        .ok_or("Missing selected entry")?;
    let item = root.children[container].as_mut_element().unwrap().children[item]
        .as_mut_element()
        .unwrap();
    for f in fields {
        let mut parts = f.path.split('@');
        let name = parts.next().unwrap();
        let attr = parts.next();
        if item.get_child(name).is_none() {
            item.children.push(XMLNode::Element(Element::new(name)));
        }
        let n = item.get_mut_child(name).unwrap();
        if let Some(a) = attr {
            n.attributes.insert(a.into(), f.value.clone());
        } else {
            n.children.clear();
            n.children.push(XMLNode::Text(f.value.clone()));
        }
    }
    write_element(&root)
}
pub fn handling(name: &str) -> String {
    let defaults = [
        ("fMass", "1800"),
        ("fInitialDragCoeff", "8"),
        ("fPercentSubmerged", "85"),
        ("fDriveBiasFront", "0"),
        ("nInitialDriveGears", "6"),
        ("fInitialDriveForce", "0.32"),
        ("fDriveInertia", "1"),
        ("fClutchChangeRateScaleUpShift", "2.5"),
        ("fClutchChangeRateScaleDownShift", "2.5"),
        ("fInitialDriveMaxFlatVel", "160"),
        ("fBrakeForce", "0.8"),
        ("fBrakeBiasFront", "0.6"),
        ("fHandBrakeForce", "0.7"),
        ("fSteeringLock", "35"),
        ("fTractionCurveMax", "2.4"),
        ("fTractionCurveMin", "2.1"),
        ("fTractionCurveLateral", "22.5"),
        ("fTractionSpringDeltaMax", "0.15"),
        ("fLowSpeedTractionLossMult", "1"),
        ("fCamberStiffnesss", "0"),
        ("fTractionBiasFront", "0.48"),
        ("fTractionLossMult", "1"),
        ("fSuspensionForce", "2.2"),
        ("fSuspensionCompDamp", "1.2"),
        ("fSuspensionReboundDamp", "2"),
        ("fSuspensionUpperLimit", "0.1"),
        ("fSuspensionLowerLimit", "-0.1"),
        ("fSuspensionRaise", "0"),
        ("fSuspensionBiasFront", "0.5"),
        ("fAntiRollBarForce", "0.8"),
        ("fAntiRollBarBiasFront", "0.5"),
        ("fRollCentreHeightFront", "0.3"),
        ("fRollCentreHeightRear", "0.3"),
        ("fCollisionDamageMult", "0.7"),
        ("fWeaponDamageMult", "1"),
        ("fDeformationDamageMult", "0.7"),
        ("fEngineDamageMult", "1.5"),
        ("fPetrolTankVolume", "65"),
        ("fOilVolume", "5"),
        ("fSeatOffsetDistX", "0"),
        ("fSeatOffsetDistY", "0"),
        ("fSeatOffsetDistZ", "0"),
        ("nMonetaryValue", "25000"),
    ];
    let fields = defaults
        .iter()
        .map(|(k, v)| format!("<{k} value=\"{v}\"/>"))
        .collect::<String>();
    format!(
        "<CHandlingDataMgr><HandlingData><Item type=\"CHandlingData\"><handlingName>{}</handlingName>{fields}<vecCentreOfMassOffset x=\"0\" y=\"0\" z=\"-0.1\"/><vecInertiaMultiplier x=\"1\" y=\"1.3\" z=\"1.5\"/><strModelFlags>440010</strModelFlags><strHandlingFlags>0</strHandlingFlags><strDamageFlags>0</strDamageFlags><AIHandling>AVERAGE</AIHandling><SubHandlingData><Item type=\"CCarHandlingData\"><fBackEndPopUpCarImpulseMult value=\"0.1\"/><fBackEndPopUpBuildingImpulseMult value=\"0.03\"/><fBackEndPopUpMaxDeltaSpeed value=\"0.6\"/></Item><Item type=\"NULL\"/><Item type=\"NULL\"/></SubHandlingData></Item></HandlingData></CHandlingDataMgr>",
        xml_escape(name)
    )
}
pub fn vehicles(model: &str, handling: &str) -> String {
    let texts = [
        ("modelName", model),
        ("txdName", model),
        ("handlingId", handling),
        ("gameName", model),
        ("vehicleMakeName", ""),
        ("expressionDictName", "null"),
        ("expressionName", "null"),
        ("animConvRoofDictName", "null"),
        ("animConvRoofName", "null"),
        ("animConvRoofWindowsAffected", ""),
        ("ptfxAssetName", "null"),
        ("audioNameHash", "POLICE"),
        ("layout", "LAYOUT_STANDARD"),
        ("coverBoundOffsets", "POLICE_COVER_OFFSET_INFO"),
        ("explosionInfo", "EXPLOSION_INFO_DEFAULT"),
        ("scenarioLayout", ""),
        ("cameraName", "DEFAULT_FOLLOW_VEHICLE_CAMERA"),
        ("aimCameraName", "DEFAULT_THIRD_PERSON_VEHICLE_AIM_CAMERA"),
        ("bonnetCameraName", "VEHICLE_BONNET_CAMERA_STANDARD"),
        ("povCameraName", "DEFAULT_POV_CAMERA"),
        ("vfxInfoName", "VFXVEHICLEINFO_CAR_GENERIC"),
        ("flags", "FLAG_EMERGENCY_SERVICE FLAG_HAS_LIVERY"),
        ("type", "VEHICLE_TYPE_CAR"),
        ("plateType", "VPT_FRONT_AND_BACK_PLATES"),
        ("dashboardType", "VDT_POLICE"),
        ("vehicleClass", "VC_EMERGENCY"),
        ("wheelType", "VWT_SPORT"),
        ("nmBraceOverrideSet", ""),
        ("drivers", ""),
        ("extraIncludes", ""),
        ("trailers", ""),
        ("additionalTrailers", ""),
        ("doorsWithCollisionWhenClosed", ""),
        ("driveableDoors", ""),
        ("bumpersNeedToCollideWithMap", ""),
        ("requiredExtras", ""),
        ("rewards", ""),
        ("cinematicPartCamera", ""),
    ];
    let mut fields = texts
        .iter()
        .map(|(k, v)| format!("<{k}>{}</{k}>", xml_escape(v)))
        .collect::<String>();
    for k in [
        "FirstPersonDriveByIKOffset",
        "FirstPersonDriveByUnarmedIKOffset",
        "FirstPersonProjectileDriveByIKOffset",
        "FirstPersonProjectileDriveByPassengerIKOffset",
        "FirstPersonProjectileDriveByRearLeftIKOffset",
        "FirstPersonProjectileDriveByRearRightIKOffset",
        "FirstPersonDriveByLeftPassengerIKOffset",
        "FirstPersonDriveByRightPassengerIKOffset",
        "FirstPersonDriveByRightRearPassengerIKOffset",
        "FirstPersonDriveByLeftRearPassengerIKOffset",
        "FirstPersonDriveByLeftPassengerUnarmedIKOffset",
        "FirstPersonDriveByRightPassengerUnarmedIKOffset",
        "PovPassengerCameraOffset",
        "PovRearPassengerCameraOffset",
    ] {
        fields.push_str(&format!("<{k} x=\"0\" y=\"0\" z=\"0\"/>"))
    }
    fields.push_str("<FirstPersonMobilePhoneOffset x=\"0.155\" y=\"0.315\" z=\"0.52\"/><FirstPersonPassengerMobilePhoneOffset x=\"0.155\" y=\"0.315\" z=\"0.52\"/><PovCameraOffset x=\"0\" y=\"-0.2\" z=\"0.65\"/>");
    for (k, v) in [
        ("PovCameraRollInfluence", "1"),
        ("povCameraVerticalAdjustmentForRollCage", "0"),
        ("povCameraHorizontalAdjustmentForRollCage", "0"),
        ("povCameraHorizontalAdjustmentForRollCageRear", "0"),
        ("wheelScale", "0.25"),
        ("wheelScaleRear", "0.25"),
        ("dirtLevelMin", "0"),
        ("dirtLevelMax", "1"),
        ("envEffScaleMin", "0"),
        ("envEffScaleMax", "1"),
        ("envEffScaleMin2", "0"),
        ("envEffScaleMax2", "1"),
        ("damageMapScale", "0.6"),
        ("damageOffsetScale", "1"),
        ("diffuseTint", "0x00FFFFFF"),
        ("steerWheelMult", "1"),
        ("HDTextureDist", "5"),
        ("minSeatHeight", "0.83"),
        ("identicalModelSpawnDistance", "20"),
        ("maxNumOfSameColor", "10"),
        ("defaultBodyHealth", "1000"),
        ("frequency", "100"),
        ("swankness", "3"),
        ("maxNum", "999"),
        ("buoyancySphereSizeScale", "1"),
        ("buoyancySphereOffset", "0"),
        ("useCinematicViewMode", "true"),
        ("useDynamicDoorScenario", "false"),
        ("povCameraAllowOneHandedDriving", "true"),
        ("shouldUseCinematicVehicleIdleCamera", "false"),
        ("shouldCameraTransitionOnClimbUpDown", "false"),
        ("shouldCameraIgnoreExiting", "false"),
        ("AllowPretendOccupants", "true"),
        ("AllowJoyriding", "true"),
        ("AllowSundayDriving", "true"),
        ("AllowBodyColorMapping", "true"),
    ] {
        fields.push_str(&format!("<{k} value=\"{v}\"/>"))
    }
    format!(
        "<CVehicleModelInfo__InitDataList><residentTxd>vehshare</residentTxd><residentAnims/><InitDatas><Item>{fields}<lodDistances content=\"float_array\">15 30 60 120 500 500</lodDistances><firstPersonDrivebyData><Item>STD_POLICE_FRONT_LEFT</Item><Item>STD_POLICE_FRONT_RIGHT</Item></firstPersonDrivebyData></Item></InitDatas><txdRelationships><Item><parent>vehshare</parent><child>{}</child></Item></txdRelationships></CVehicleModelInfo__InitDataList>",
        xml_escape(model)
    )
}
pub fn variations(model: &str, siren: u32, light: u16) -> String {
    format!(
        "<CVehicleModelInfoVariation><variationData><Item><modelName>{}</modelName><colors><Item><indices content=\"char_array\">0 0 0 0 0 0</indices><liveries><Item value=\"true\"/></liveries></Item></colors><kits><Item>0_default_modkit</Item></kits><windowsWithExposedEdges/><plateProbabilities><Probabilities><Item><Name>Standard White</Name><Value value=\"100\"/></Item></Probabilities></plateProbabilities><lightSettings value=\"{light}\"/><sirenSettings value=\"{siren}\"/></Item></variationData></CVehicleModelInfoVariation>",
        xml_escape(model)
    )
}
pub fn manifest(model: &str) -> Result<String> {
    if model.is_empty()
        || !model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Resource name must use letters, digits, underscores or hyphens.".into());
    }
    Ok(format!(
        "fx_version 'cerulean'\ngame 'gta5'\n\nauthor 'Duck Workshop'\ndescription '{model} vehicle metadata'\n\nfiles {{ 'data/*.meta' }}\ndata_file 'HANDLING_FILE' 'data/handling.meta'\ndata_file 'VEHICLE_METADATA_FILE' 'data/vehicles.meta'\ndata_file 'CARCOLS_FILE' 'data/carcols.meta'\ndata_file 'VEHICLE_VARIATION_FILE' 'data/carvariations.meta'\n"
    ))
}
