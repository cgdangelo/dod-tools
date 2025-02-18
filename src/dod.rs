use dem::types::UserMessage;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    combinator::{all_consuming, eof, fail, opt, success},
    multi::{length_count, many0},
    number::complete::{le_i16, le_i8, le_u16, le_u8},
    sequence::terminated,
    IResult, Parser,
};
use std::time::Duration;

#[derive(Debug)]
pub enum Message {
    AmmoPickup(AmmoPickup),
    AmmoShort(AmmoShort),
    AmmoX(AmmoX),
    BloodPuff(BloodPuff),
    CameraView(CameraView),
    CancelProg(CancelProg),
    CapMsg(CapMsg),
    ClanTimer(ClanTimer),
    ClCorpse(ClCorpse),
    ClientAreas(ClientAreas),
    CurMarker(CurMarker),
    CurWeapon(CurWeapon),
    DeathMsg(DeathMsg),
    Frags(Frags),
    GameRules(GameRules),
    HandSignal(HandSignal),
    Health(Health),
    HideWeapon(HideWeapon),
    HLTV(HLTV),
    HudText(HudText),
    InitHUD(InitHUD),
    InitObj(InitObj),
    MapMarker(MapMarker),
    MOTD(MOTD),
    ObjScore(ObjScore),
    Object(Object),
    PClass(PClass),
    PShoot(PShoot),
    PStatus(PStatus),
    PTeam(PTeam),
    PlayersIn(PlayersIn),
    ReloadDone(ReloadDone),
    ResetHUD(ResetHUD),
    ResetSens(ResetSens),
    RoundState(RoundState),
    SayText(SayText),
    Scope(Scope),
    ScoreShort(ScoreShort),
    ScreenFade(ScreenFade),
    ScreenShake(ScreenShake),
    ServerName(ServerName),
    SetFOV(SetFOV),
    SetObj(SetObj),
    ShowMenu(ShowMenu),
    Spectator(Spectator),
    StartProg(StartProg),
    StartProgF(StartProgF),
    StatusValue(StatusValue),
    TeamScore(TeamScore),
    TextMsg(TextMsg),
    TimeLeft(TimeLeft),
    TimerStatus(TimerStatus),
    UseSound(UseSound),
    VGUIMenu(VGUIMenu),
    VoiceMask(VoiceMask),
    WaveStatus(WaveStatus),
    WaveTime(WaveTime),
    WeaponList(WeaponList),
    WeapPickup(WeapPickup),
    Weather(Weather),
    YouDied(YouDied),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Team {
    Allies,
    Axis,
    Spectators,
}

#[derive(Debug)]
pub enum Class {
    AxisMortar,
    Bazooka,
    BritishMortar,
    BritishRifleman,
    Fg42Zielfernrohr,
    Fg42Zweibein,
    Grenadier,
    Gunner,
    MG34Schutze,
    MG42Schutze,
    MachineGunner,
    Marksman,
    MasterSergeant,
    Mortar,
    Panzerschreck,
    Random,
    Rifleman,
    RocketInfantry,
    Scharfschutze,
    Sergeant,
    SergeantMajor,
    Sniper,
    StaffSergeant,
    Stosstruppe,
    Sturmtruppe,
    SupportInfantry,
    Unteroffizer,
}

#[derive(Debug)]
pub enum Weapon {
    Kabar = 1,
    GermanKnife = 2,
    M1911 = 3,
    Luger = 4,
    Garand = 5,
    ScopedK98 = 6,
    Thompson = 7,
    Stg44 = 8,
    Springfield = 9,
    K98 = 10,
    Bar = 11,
    Mp40 = 12,
    Mk2Grenade = 13,
    StickGrenade = 14,
    // 15 ? Something related to grenades...
    // 16 ? Something related to grenades...
    Mg42 = 17,
    Browning30Cal = 18,
    Spade = 19,
    M1Carbine = 20,
    Mg34 = 21,
    GreaseGun = 22,
    Fg42 = 23,
    K43 = 24,
    LeeEnfield = 25,
    Sten = 26,
    Bren = 27,
    Webley = 28,
    Bazooka = 29,
    Panzerschreck = 30,
    Piat = 31,
    Mortar = 32,
    // 33 ?
    // 34 ?
    ScopedFg42 = 35,
    M1A1Carbine = 36,
    K98Bayonet = 37,
    ScopedLeeEnfield = 38,
    MillsBomb = 39,
    BritishKnife = 40,
    // 41 ?
    ButtStock = 42, // Same id for Garand/K43
    EnfieldBayonet = 43,
}

/// - Length: 2
#[derive(Debug)]
pub struct AmmoPickup {}

#[derive(Debug)]
pub struct AmmoShort {
    pub ammo_id: u8,
    pub amount: i16,
}

#[derive(Debug)]
pub struct AmmoX {
    pub ammo_id: u8,
    pub amount: u8,
}

/// Sent when a blood sprite should be rendered.
#[derive(Debug)]
pub struct BloodPuff(pub (i16, i16, i16));

#[derive(Debug)]
pub struct CameraView {
    pub target_name: String,
}

/// Sent when objective capture is interrupted.
#[derive(Debug)]
pub struct CancelProg {
    pub area_index: u8,
    _unk2: u8,
}

/// Sent when an objective is captured by a player.
#[derive(Debug)]
pub struct CapMsg {
    pub client_index: u8,
    pub point_name: String,
    pub team: Team,
}

/// Sent when the countdown to a clan match begins.
///
/// - Value: `mp_clan_timer`
#[derive(Debug)]
pub struct ClanTimer(pub Duration);

/// - Frequency: unknown trigger; often
/// - Length: variable
#[derive(Debug)]
pub struct ClCorpse {
    pub model_name: String,
    pub origin: (i16, i16, i16),
    pub angle: (i8, i8, i8),
    pub animation_sequence: u8,
    pub body: u16,
    pub team: Team,
}

/// - Frequency: unknown trigger; often in POV, once in HLTV
/// - Length: variable; often 2
/// - Values:
///     CAreaCapture::area_SendStatus = {m_iAreaIndex, -1, sz_HudIcon}
///     CAreaCapture::area_SetIndex = {m_iAreaIndex, -1, sz_HudIcon}
///     CBasePlayer::HandleSignals = {m_iCapAreaIconIndex, 0}
///     CBasePlayer::HandleSignals = {m_iObjectAreaIndex, 0}
///     CBasePlayer::SetClientAreaIcon = {int, bool}
///     CBreakable::area_SendStatus = {m_iAreaIndex, -1, sz_HudIcon}
///     CBreakable::area_SetIndex = {m_iAreaIndex, -1, sz_HudIcon}
///     CDoDTeamPlay::LevelChangeResets = {0, 2}
///     CObjectCapture::area_SendStatus = {m_iAreaIndex, -1, sz_HudIcon}
///     CObjectCapture::area_SetIndex = {m_iAreaIndex, -1, sz_HudIcon}
#[derive(Debug)]
pub struct ClientAreas {
    pub icon_index: u8,
    pub hud_icon: Option<String>,
}

/// - Length: 1
#[derive(Debug)]
pub struct CurMarker {
    pub marker_id: u8,
}

/// - Length: 3
#[derive(Debug)]
pub struct CurWeapon {
    pub is_active: bool,
    pub weapon: Weapon,
    pub clip_ammo: u8,
}

/// Sent when a player kills another player to rerender the HUD.
#[derive(Debug)]
pub struct DeathMsg {
    /// Client index of the killer, or 0 if the death was a suicide.
    pub killer_client_index: u8,

    /// Client index of the victim.
    pub victim_client_index: u8,

    /// Weapon that was used to kill the victim.
    pub weapon: Weapon,
}

/// Sent when a player's frag total has updated.
#[derive(Debug)]
pub struct Frags {
    /// Client index of the player.
    pub client_index: u8,

    /// New total number of frags by the player.
    pub frags: i16,
}

/// - Frequency: 1 each spawn in POV; 1 on connection in HLTV?
/// - Length: 2
/// - Value: likely bitfields = {allies_rules, axis_rules}
///
///  struct gameplay_rules_t // sizeof=0x30
///  {                                       // XREF: CDoDTeamPlay/r
///                                          // CDoDDetect/r
///      bool m_bAlliesInfiniteLives;
///      bool m_bAxisInfiniteLives;
///      bool m_bAlliesArePara;
///      bool m_bAxisArePara;
///      bool m_bAlliesAreBrit;
#[derive(Debug)]
pub struct GameRules {
    _unk1: u8,
    _unk2: u8,
}

/// Sent when a player should play a hand signal animation.
///
/// - Length: 2
/// - Value: (client_index, animation_id)?
#[derive(Debug)]
pub struct HandSignal {
    pub client_index: u8,
    pub animation_id: u8,
}

/// Sent when the POV's health changes to rerender the HUD.
///
/// - Length: 1
/// - Value: 0 - 100
#[derive(Debug)]
pub struct Health(pub u8);

/// Sent when the POV's weapon should be holstered or removed from holster.
///
/// - Length: 1
/// - Value: 9 after YouDied; 0 otherwise
#[derive(Debug)]
pub struct HideWeapon {}

/// - Length: 2
#[derive(Debug)]
pub struct HLTV {}

#[derive(Debug)]
pub struct HudText {
    pub text: String,
    pub init_hud_style: u8,
}

/// Sent when the POV joins the game to prepare their HUD.
///
/// - Frequency: 1 on connection
/// - Length: 0
#[derive(Debug)]
pub struct InitHUD {}

#[derive(Debug)]
pub struct Objective {
    pub entity_index: u16,
    pub area_index: u8,
    pub team: Option<Team>, // u8
    pub _unk1: u8,
    pub neutral_icon_index: u8,
    pub allies_icon_index: u8,
    pub axis_icon_index: u8,
    pub origin: (u8, u8),
}

/// - Length: varies
#[derive(Debug)]
pub struct InitObj {
    pub objectives: Vec<Objective>,
}

/// - Length: 6
#[derive(Debug)]
pub struct MapMarker {}

/// Sent when the POV connects to the server so it can render the MOTD window.
#[derive(Debug)]
pub struct MOTD {
    /// True if there are no more MOTD messages following this one, false otherwise.
    pub is_terminal: bool,
    pub text: String,
}

/// Sent when a player's objective score changes.
#[derive(Debug)]
pub struct ObjScore {
    /// Client index of the player.
    pub client_index: u8,

    /// Amount of points the player has accrued.
    pub score: i16,
}

/// - Length: varies
#[derive(Debug)]
pub struct Object {}

/// Sent when player class changes.
#[derive(Debug)]
pub struct PClass {
    /// Client index of the player that changed class.
    pub client_index: u8,

    /// Identifier for the class chosen by the player.
    pub class: Class,
}

/// - Length: varies
///
/// ```
///   (*((void (__cdecl **)(_DWORD))&gTankSpread.has_disconnected + 48))(this->m_iGroupId);
///   (*((void (__cdecl **)(int))&gTankSpread.has_disconnected + 48))(this->m_iState);
///   if ( this->m_iState == 1 )
///   {
///     (*((void (__cdecl **)(_DWORD))&gTankSpread.has_disconnected + 53))(LODWORD(this->m_vShootDir.x));
///     (*((void (__cdecl **)(_DWORD))&gTankSpread.has_disconnected + 53))(LODWORD(this->m_vShootDir.y));
///     (*((void (__cdecl **)(_DWORD))&gTankSpread.has_disconnected + 53))(LODWORD(this->m_vShootDir.z));
///   }
/// ```
#[derive(Debug)]
pub struct PShoot {}

#[derive(Debug)]
pub struct PStatus {
    /// Client index of the updated player.
    pub client_index: u8,

    pub status: u8,
}

/// Sent when a player joins a team.
#[derive(Debug)]
pub struct PTeam {
    /// Client index of the player.
    pub client_index: u8,

    /// Identifier for the team that was joined.
    pub team: Team,
}

/// Sent when more players enter a capture area to rerender the HUD.
///
/// - Length: 4
#[derive(Debug)]
pub struct PlayersIn {
    pub area_index: u8,
    pub team: Team,
    pub players_inside_area: u8,
    pub required_players_for_area: u8,
}

/// - Length: 3
pub struct ProgUpdate {
    pub area_index: u8,
    pub team: Team,
}

/// - Length: 0
#[derive(Debug)]
pub struct ReloadDone {}

/// Sent when the POV spawns to reset their HUD state for the new life.
///
/// - Frequency: 1 on each spawn
/// - Length: 0
#[derive(Debug)]
pub struct ResetHUD {}

/// - Length: 0
#[derive(Debug)]
pub struct ResetSens {}

/// Sent when the round state changes, for example, when a team completes all objectives.
#[derive(Debug)]
pub enum RoundState {
    Reset = 0,
    Normal = 1,
    AlliesWin = 3,
    AxisWin = 4,
    Draw = 5,
}

/// Sent when a player sends a message in chat.
#[derive(Debug)]
pub struct SayText {
    pub client_index: u8,
    pub text: String,
}

/// Sent when the POV looks through a scope of a weapon.
///
/// - Length: 1
/// - Value:
///     CBasePlayerWeapon::ZoomIn = m_iId
///     CBasePlayerWeapon::ZoomOut = 0
///     CDoDTeamPlay::UpdateData
#[derive(Debug)]
pub struct Scope {}

/// Possibly deprecated and replaced by [ScoreShort].
///
/// CDoDTeamPlay::InitHUD
/// CDoDTeamPlay::UpdateData
#[derive(Debug)]
pub struct ScoreInfo {
    pub client_index: u8,
    pub points: i8,
    pub kills: i8,
    pub deaths: i8,
    pub class: Class,
    pub team: Team,
}

/// Intended to be sent when the values in a [ScoreInfo] would overflow, but [ScoreInfoLong].
///
/// - Length: 10
#[derive(Debug)]
pub struct ScoreInfoLong {
    pub client_index: u8,
    pub score: i16,
    pub frags: i16,
    pub class: Class,
    pub team: Team,
}

/// - Length: 8
#[derive(Debug)]
pub struct ScoreShort {
    pub client_index: u8,
    pub score: i16,
    pub kills: i16,
    pub deaths: i16,
}

#[derive(Debug)]
pub struct ScreenFade {
    duration: u16,
    hold_time: u16,
    flags: i16,
    color: (u8, u8, u8, u8),
}

/// Sent when the POV should render a screen shake animation, such as after a grenade
/// explosion.
///
/// - Length: 6
#[derive(Debug)]
pub struct ScreenShake {
    amplitude: u16,
    duration: u16,
    frequency: u16,
}

/// Sent when the POV connects to a server.
///
/// - Length: variable
#[derive(Debug)]
pub struct ServerName(pub String);

/// - Length: 1
#[derive(Debug)]
pub struct SetFOV(pub u8);

/// - Length: 3
/// - Value: (0..4, 0..2, 0)
#[derive(Debug)]
pub struct SetObj {
    pub client_index: u8,
    pub team: Team,
    _unk1: u8,
}

/// - Length: varies
#[derive(Debug)]
pub struct ShowMenu {}

/// Sent when a player's spectator state changes.
#[derive(Debug)]
pub struct Spectator {
    /// Client index of the player.
    pub client_index: u8,

    /// True if the player joined spectators, false if the player left spectators.
    pub is_spectator: bool,
}

/// Sent when objective capture progress has started.
///
/// - Length: 4
#[derive(Debug)]
pub struct StartProg {
    pub area_index: u8,
    pub team: Team,
    pub cap_duration: Duration, // u16
}

/// - Length: 4
#[derive(Debug)]
pub struct StartProgF {
    pub area_index: u8,
    pub team: Team,
    pub cap_duration: Duration, // f32 -> WRITE_COORD -> u16 -> f32 -> Duration
}

/// - Value: 0..100 (health?)
#[derive(Debug)]
pub struct StatusValue {}

/// Sent when a team scores points either by objective or tick.
///
/// - Length: 3
#[derive(Debug)]
pub struct TeamScore {
    pub team: Team,
    pub score: u16,
}

#[derive(Debug)]
pub struct TextMsg {
    pub destination: u8,
    pub text: String,
    pub arg1: Option<String>,
    pub arg2: Option<String>,
    pub arg3: Option<String>,
    pub arg4: Option<String>,
}

/// - Length: 2
#[derive(Debug)]
pub struct TimeLeft(pub Duration);

/// - Length: 3
#[derive(Debug)]
pub struct TimerStatus {}

/// - Length: 1
/// - Value: 0 or 1
#[derive(Debug)]
pub struct UseSound {
    pub is_button_pressed: bool,
}

/// Sent when the POV needs to render a VGUI menu?
///
/// - Length: 5
#[derive(Debug)]
pub struct VGUIMenu {}

#[derive(Debug)]
pub struct VoiceMask {}

/// Sent when the state of the reinforcements timer changes.
#[derive(Debug)]
pub struct WaveStatus(pub u8);

/// Sent when the reinforcements timer's should be set to a value.
///
/// - Value: 0 or `mp_clan_respawntime`
#[derive(Debug)]
pub struct WaveTime(pub Duration); // u8

/// - Frequency: many on POV connection?
/// - Length: varies
#[derive(Debug)]
pub struct WeaponList {
    pub primary_ammo_id: u8,
    pub primary_ammo_max: u8,
    pub secondary_ammo_id: u8,
    pub secondary_ammo_max: u8,
    pub slot: u8,
    pub position_in_slot: u8,
    pub weapon: Weapon,
    _unk1: u8,
    _unk2: u8,
    pub clip_size: u8,
}

/// - Length: 1
#[derive(Debug)]
pub struct WeapPickup {}

/// - Length: 2
#[derive(Debug)]
pub struct Weather {}

/// Sent when the POV dies.
///
/// - Length: 1
#[derive(Debug)]
pub struct YouDied {}

fn wrapped_string<T>(i: &[u8], f: fn(String) -> T) -> IResult<&[u8], T> {
    all_consuming(many0(le_u8))
        .map_res(String::from_utf8)
        .map(f)
        .parse(i)
}

fn null_string(i: &[u8]) -> IResult<&[u8], String> {
    alt((
        tag("\x00").map(|_| vec![]),
        terminated(take_until("\x00"), tag("\x00")).map(Vec::from),
    ))
    .map_res(String::from_utf8)
    .parse(i)
}

fn class(i: &[u8]) -> IResult<&[u8], Class> {
    le_u8
        .map_res(|value| match value {
            // FIXME Inaccurate!
            1 => Ok(Class::Rifleman),
            2 => Ok(Class::StaffSergeant),
            3 => Ok(Class::MasterSergeant),
            4 => Ok(Class::Sergeant),
            5 => Ok(Class::Sniper),
            6 => Ok(Class::SupportInfantry),
            7 => Ok(Class::MachineGunner),
            8 => Ok(Class::Bazooka),
            9 => Ok(Class::Mortar),
            10 => Ok(Class::Grenadier),
            11 => Ok(Class::Stosstruppe),
            12 => Ok(Class::Unteroffizer),
            13 => Ok(Class::Sturmtruppe),
            14 => Ok(Class::Scharfschutze),
            15 => Ok(Class::Fg42Zweibein),
            16 => Ok(Class::Fg42Zielfernrohr),
            17 => Ok(Class::MG34Schutze),
            18 => Ok(Class::MG42Schutze),
            19 => Ok(Class::Panzerschreck),
            20 => Ok(Class::AxisMortar),
            21 => Ok(Class::BritishRifleman),
            22 => Ok(Class::SergeantMajor),
            23 => Ok(Class::Marksman),
            24 => Ok(Class::Gunner),
            25 => Ok(Class::RocketInfantry),
            26 => Ok(Class::BritishMortar),
            27 => Ok(Class::Random),
            _ => Err(()),
        })
        .parse(i)
}

fn team(i: &[u8]) -> IResult<&[u8], Team> {
    le_u8
        .map_res(|value| match value {
            1 => Ok(Team::Allies),
            2 => Ok(Team::Axis),
            3 => Ok(Team::Spectators),
            _ => Err(()),
        })
        .parse(i)
}

fn weapon(i: &[u8]) -> IResult<&[u8], Weapon> {
    le_u8
        .map_res(|value| match value {
            1 => Ok(Weapon::Kabar),
            2 => Ok(Weapon::GermanKnife),
            3 => Ok(Weapon::M1911),
            4 => Ok(Weapon::Luger),
            5 => Ok(Weapon::Garand),
            6 => Ok(Weapon::ScopedK98),
            7 => Ok(Weapon::Thompson),
            8 => Ok(Weapon::Stg44),
            9 => Ok(Weapon::Springfield),
            10 => Ok(Weapon::K98),
            11 => Ok(Weapon::Bar),
            12 => Ok(Weapon::Mp40),
            13 => Ok(Weapon::Mk2Grenade),
            14 => Ok(Weapon::StickGrenade),
            17 => Ok(Weapon::Mg42),
            18 => Ok(Weapon::Browning30Cal),
            19 => Ok(Weapon::Spade),
            20 => Ok(Weapon::M1Carbine),
            21 => Ok(Weapon::Mg34),
            22 => Ok(Weapon::GreaseGun),
            23 => Ok(Weapon::Fg42),
            24 => Ok(Weapon::K43),
            25 => Ok(Weapon::LeeEnfield),
            26 => Ok(Weapon::Sten),
            27 => Ok(Weapon::Bren),
            28 => Ok(Weapon::Webley),
            29 => Ok(Weapon::Bazooka),
            30 => Ok(Weapon::Panzerschreck),
            31 => Ok(Weapon::Piat),
            32 => Ok(Weapon::Mortar),
            35 => Ok(Weapon::ScopedFg42),
            36 => Ok(Weapon::M1A1Carbine),
            37 => Ok(Weapon::K98Bayonet),
            38 => Ok(Weapon::ScopedLeeEnfield),
            39 => Ok(Weapon::MillsBomb),
            40 => Ok(Weapon::BritishKnife),
            42 => Ok(Weapon::ButtStock),
            43 => Ok(Weapon::EnfieldBayonet),
            _ => Err(()),
        })
        .parse(i)
}

impl TryFrom<&UserMessage> for Message {
    type Error = ();

    fn try_from(value: &UserMessage) -> Result<Self, Self::Error> {
        let message_name = String::from_utf8(value.name.to_vec()).map_err(|_| ())?;
        let message_name = message_name.trim_end_matches('\x00');

        let i = &value.data;

        let (_, message) = match message_name {
            "AmmoX" => ammox.map(Self::AmmoX).parse(i),
            "BloodPuff" => blood_puff.map(Self::BloodPuff).parse(i),
            "CancelProg" => cancel_prog.map(Self::CancelProg).parse(i),
            "CapMsg" => cap_msg.map(Self::CapMsg).parse(i),
            "ClCorpse" => cl_corpse.map(Self::ClCorpse).parse(i),
            "ClanTimer" => clan_timer.map(Self::ClanTimer).parse(i),
            "ClientAreas" => client_areas.map(Self::ClientAreas).parse(i),
            "CurWeapon" => cur_weapon.map(Self::CurWeapon).parse(i),
            "DeathMsg" => death_msg.map(Self::DeathMsg).parse(i),
            "Frags" => frags.map(Self::Frags).parse(i),
            "Health" => health.map(Self::Health).parse(i),
            "HudText" => hud_text.map(Self::HudText).parse(i),
            "InitHUD" => init_hud.map(Self::InitHUD).parse(i),
            "InitObj" => init_obj.map(Self::InitObj).parse(i),
            "MOTD" => motd.map(Self::MOTD).parse(i),
            "ObjScore" => obj_score.map(Self::ObjScore).parse(i),
            "PClass" => p_class.map(Self::PClass).parse(i),
            "PStatus" => p_status.map(Self::PStatus).parse(i),
            "PTeam" => p_team.map(Self::PTeam).parse(i),
            "PlayersIn" => players_in.map(Self::PlayersIn).parse(i),
            "ReloadDone" => reload_done.map(Self::ReloadDone).parse(i),
            "ResetHUD" => reset_hud.map(Self::ResetHUD).parse(i),
            "ResetSens" => reset_sens.map(Self::ResetSens).parse(i),
            "RoundState" => round_state.map(Self::RoundState).parse(i),
            "SayText" => say_text.map(Self::SayText).parse(i),
            "Scope" => scope.map(Self::Scope).parse(i),
            "ScoreShort" => score_short.map(Self::ScoreShort).parse(i),
            "ServerName" => server_name.map(Self::ServerName).parse(i),
            "SetFOV" => set_fov.map(Self::SetFOV).parse(i),
            "Spectator" => spectator.map(Self::Spectator).parse(i),
            "StartProg" => start_prog.map(Self::StartProg).parse(i),
            "TeamScore" => team_score.map(Self::TeamScore).parse(i),
            "TextMsg" => text_msg.map(Self::TextMsg).parse(i),
            "TimeLeft" => time_left.map(Self::TimeLeft).parse(i),
            "UseSound" => use_sound.map(Self::UseSound).parse(i),
            "WaveStatus" => wave_status.map(Self::WaveStatus).parse(i),
            "WaveTime" => wave_time.map(Self::WaveTime).parse(i),
            "WeaponList" => weapon_list.map(Self::WeaponList).parse(i),
            _ => fail::<&[u8], Message, _>().parse(i),
        }
        .map_err(|e| {
            // println!("{} {:?} {:?}", message_name, e, value.data);

            ()
        })?;

        Ok(message)
    }
}

impl TryFrom<&str> for Team {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "allies" => Ok(Team::Allies),
            "axis" => Ok(Team::Axis),
            "spectators" => Ok(Team::Spectators),
            _ => Err(()),
        }
    }
}

fn ammox(i: &[u8]) -> IResult<&[u8], AmmoX> {
    all_consuming((le_u8, le_u8))
        .map(|(ammo_id, amount)| AmmoX { ammo_id, amount })
        .parse(i)
}

fn blood_puff(i: &[u8]) -> IResult<&[u8], BloodPuff> {
    all_consuming((le_i16, le_i16, le_i16))
        .map(BloodPuff)
        .parse(i)
}

fn cancel_prog(i: &[u8]) -> IResult<&[u8], CancelProg> {
    all_consuming((le_u8, le_u8))
        .map(|(area_index, _unk2)| CancelProg { area_index, _unk2 })
        .parse(i)
}

fn cap_msg(i: &[u8]) -> IResult<&[u8], CapMsg> {
    all_consuming((le_u8, null_string, team))
        .map(|(client_index, point_name, team)| CapMsg {
            client_index,
            point_name,
            team,
        })
        .parse(i)
}

fn cl_corpse(i: &[u8]) -> IResult<&[u8], ClCorpse> {
    all_consuming((
        null_string,
        (le_i16, le_i16, le_i16),
        (le_i8, le_i8, le_i8),
        le_u8,
        le_u16,
        team,
    ))
    .map(
        |(model_name, origin, angle, animation_sequence, body, team)| ClCorpse {
            model_name,
            origin,
            angle,
            animation_sequence,
            body,
            team,
        },
    )
    .parse(i)
}

fn clan_timer(i: &[u8]) -> IResult<&[u8], ClanTimer> {
    le_u8
        .map(|clan_timer_seconds| {
            let duration = Duration::from_secs(clan_timer_seconds as u64);

            ClanTimer(duration)
        })
        .parse(i)
}

fn client_areas(i: &[u8]) -> IResult<&[u8], ClientAreas> {
    let (i, icon_index) = le_u8(i)?;
    let (i, flags) = le_u8(i)?;

    let (i, hud_icon) = match flags {
        255 => null_string.map(Some).parse(i)?,
        _ => success(None).parse(i)?,
    };

    Ok((
        i,
        ClientAreas {
            icon_index,
            hud_icon,
        },
    ))

    // (le_u8, le_u8.and_then(|a| success(a)))
    //     .map(|_| ClientAreas {
    //         icon_index: 0,
    //         _unk2: 0,
    //         hud_icon: "".to_string(),
    //     })
    //     .parse(i)

    // (le_u8, le_u8, null_string, le_u8)
    //     .map(|(icon_index, _unk2, hud_icon, _)| ClientAreas {
    //         icon_index,
    //         _unk2,
    //         hud_icon,
    //     })
    //     .parse(i)
}

fn cur_weapon(i: &[u8]) -> IResult<&[u8], CurWeapon> {
    all_consuming((le_u8.map(|v| v != 0), weapon, le_u8))
        .map(|(is_active, weapon, clip_ammo)| CurWeapon {
            is_active,
            weapon,
            clip_ammo,
        })
        .parse(i)
}

fn death_msg(i: &[u8]) -> IResult<&[u8], DeathMsg> {
    all_consuming((le_u8, le_u8, weapon))
        .map(
            |(killer_client_index, victim_client_index, weapon)| DeathMsg {
                killer_client_index,
                victim_client_index,
                weapon,
            },
        )
        .parse(i)
}

fn frags(i: &[u8]) -> IResult<&[u8], Frags> {
    all_consuming((le_u8, le_i16))
        .map(|(client_index, frags)| Frags {
            client_index,
            frags,
        })
        .parse(i)
}

fn health(i: &[u8]) -> IResult<&[u8], Health> {
    all_consuming(le_u8).map(Health).parse(i)
}

fn hud_text(i: &[u8]) -> IResult<&[u8], HudText> {
    all_consuming((null_string, le_u8))
        .map(|(text, init_hud_style)| HudText {
            text,
            init_hud_style,
        })
        .parse(i)
}

fn init_hud(i: &[u8]) -> IResult<&[u8], InitHUD> {
    eof.map(|_| InitHUD {}).parse(i)
}

fn init_obj(i: &[u8]) -> IResult<&[u8], InitObj> {
    // likely inaccurate?
    let objective = |i| -> IResult<&[u8], Objective> {
        (
            le_u16,
            le_u8,
            opt(team),
            le_u8,
            le_u8,
            le_u8,
            le_u8,
            (le_u8, le_u8),
        )
            .map(
                |(
                    entity_index,
                    area_index,
                    team,
                    _unk1,
                    neutral_icon_index,
                    allies_icon_index,
                    axis_icon_index,
                    origin,
                )| Objective {
                    entity_index,
                    area_index,
                    team,
                    _unk1, // ! (spawnflags & 1)
                    neutral_icon_index,
                    allies_icon_index,
                    axis_icon_index,
                    origin,
                },
            )
            .parse(i)
    };

    all_consuming(length_count(le_u8, objective))
        .map(|objectives| InitObj { objectives })
        .parse(i)
}

fn motd(i: &[u8]) -> IResult<&[u8], MOTD> {
    all_consuming((
        le_u8.map(|v| v != 0),
        many0(le_u8).map_res(String::from_utf8),
    ))
    .map(|(is_terminal, text)| MOTD { is_terminal, text })
    .parse(i)
}

fn obj_score(i: &[u8]) -> IResult<&[u8], ObjScore> {
    all_consuming((le_u8, le_i16))
        .map(|(client_index, score)| ObjScore {
            client_index,
            score,
        })
        .parse(i)
}

fn p_class(i: &[u8]) -> IResult<&[u8], PClass> {
    (le_u8, class)
        .map(|(client_index, class)| PClass {
            client_index,
            class,
        })
        .parse(i)
}

fn p_status(i: &[u8]) -> IResult<&[u8], PStatus> {
    all_consuming((le_u8, le_u8))
        .map(|(client_index, status)| PStatus {
            client_index,
            status,
        })
        .parse(i)
}

fn p_team(i: &[u8]) -> IResult<&[u8], PTeam> {
    all_consuming((le_u8, team))
        .map(|(client_index, team)| PTeam { client_index, team })
        .parse(i)
}

fn players_in(i: &[u8]) -> IResult<&[u8], PlayersIn> {
    all_consuming((le_u8, team, le_u8, le_u8))
        .map(
            |(area_index, team, players_inside_area, required_players_for_area)| PlayersIn {
                area_index,
                team,
                players_inside_area,
                required_players_for_area,
            },
        )
        .parse(i)
}

fn reload_done(i: &[u8]) -> IResult<&[u8], ReloadDone> {
    eof.map(|_| ReloadDone {}).parse(i)
}

fn reset_hud(i: &[u8]) -> IResult<&[u8], ResetHUD> {
    eof.map(|_| ResetHUD {}).parse(i)
}

fn reset_sens(i: &[u8]) -> IResult<&[u8], ResetSens> {
    eof.map(|_| ResetSens {}).parse(i)
}

fn round_state(i: &[u8]) -> IResult<&[u8], RoundState> {
    le_u8
        .map_res(|team_id| match team_id {
            0 => Ok(RoundState::Reset),
            1 => Ok(RoundState::Normal),
            3 => Ok(RoundState::AlliesWin),
            4 => Ok(RoundState::AxisWin),
            5 => Ok(RoundState::Draw),
            _ => Err(()),
        })
        .parse(i)
}

fn say_text(i: &[u8]) -> IResult<&[u8], SayText> {
    all_consuming((
        le_u8,
        le_u8, // unk
        null_string,
    ))
    .map(|(client_index, _, text)| SayText { client_index, text })
    .parse(i)
}

fn scope(i: &[u8]) -> IResult<&[u8], Scope> {
    all_consuming(le_u8).map(|_| Scope {}).parse(i)
}

fn score_short(i: &[u8]) -> IResult<&[u8], ScoreShort> {
    all_consuming((le_u8, le_i16, le_i16, le_i16, le_u8))
        .map(|(client_index, score, kills, deaths, _)| ScoreShort {
            client_index,
            score,
            kills,
            deaths,
        })
        .parse(i)
}

fn server_name(i: &[u8]) -> IResult<&[u8], ServerName> {
    wrapped_string(i, ServerName)
}

fn set_fov(i: &[u8]) -> IResult<&[u8], SetFOV> {
    le_u8.map(SetFOV).parse(i)
}

fn spectator(i: &[u8]) -> IResult<&[u8], Spectator> {
    all_consuming((le_u8, le_u8.map(|v| v != 0)))
        .map(|(client_index, is_spectator)| Spectator {
            client_index,
            is_spectator,
        })
        .parse(i)
}

fn start_prog(i: &[u8]) -> IResult<&[u8], StartProg> {
    all_consuming((le_u8, team, le_u16.map(|v| Duration::from_secs(v as u64))))
        .map(|(area_index, team, cap_duration)| StartProg {
            area_index,
            team,
            cap_duration,
        })
        .parse(i)
}

fn team_score(i: &[u8]) -> IResult<&[u8], TeamScore> {
    all_consuming((team, le_u16))
        .map(|(team, score)| TeamScore { team, score })
        .parse(i)
}

fn text_msg(i: &[u8]) -> IResult<&[u8], TextMsg> {
    let (i, text_msg) = all_consuming((
        le_u8,
        null_string,
        opt(null_string),
        opt(null_string),
        opt(null_string),
        opt(null_string),
    ))
    .map(|(destination, text, arg1, arg2, arg3, arg4)| TextMsg {
        destination,
        text,
        arg1,
        arg2,
        arg3,
        arg4,
    })
    .parse(i)?;

    Ok((i, text_msg))
}

fn time_left(i: &[u8]) -> IResult<&[u8], TimeLeft> {
    all_consuming(le_u16)
        .map(|x| Duration::from_secs(x as u64))
        .map(TimeLeft)
        .parse(i)
}

fn use_sound(i: &[u8]) -> IResult<&[u8], UseSound> {
    all_consuming(le_u8.map(|v| v != 0))
        .map(|is_button_pressed| UseSound { is_button_pressed })
        .parse(i)
}

fn wave_status(i: &[u8]) -> IResult<&[u8], WaveStatus> {
    all_consuming(le_u8).map(WaveStatus).parse(i)
}

fn wave_time(i: &[u8]) -> IResult<&[u8], WaveTime> {
    le_u8
        .map(|seconds| {
            let duration = Duration::from_secs(seconds as u64);

            WaveTime(duration)
        })
        .parse(i)
}

fn weapon_list(i: &[u8]) -> IResult<&[u8], WeaponList> {
    all_consuming((
        le_u8, le_u8, le_u8, le_u8, le_u8, le_u8, weapon, le_u8, le_u8, le_u8,
    ))
    .map(
        |(
            primary_ammo_id,
            primary_ammo_max,
            secondary_ammo_id,
            secondary_ammo_max,
            slot,
            position_in_slot,
            weapon,
            _unk1,
            _unk2,
            clip_size,
        )| {
            WeaponList {
                primary_ammo_id,
                primary_ammo_max,
                secondary_ammo_id,
                secondary_ammo_max,
                slot,
                position_in_slot,
                weapon,
                _unk1,
                _unk2,
                clip_size,
            }
        },
    )
    .parse(i)
}
