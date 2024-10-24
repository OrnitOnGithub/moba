use top_down_shooter::common::*;
use core::f32;
use std::collections::HashMap;

use std::net::UdpSocket;
use std::sync::{Arc, Mutex, MutexGuard};
use bincode;
use std::{thread, time::*};

fn main() {

  // Load character properties
  let characters: HashMap<Character, CharacterProperties> = load_characters();
  println!("Loaded character properties.");


  let players: Vec<ServerPlayer> = Vec::new();
  let players = Arc::new(Mutex::new(players));
  let game_objects:Vec<GameObject> = load_map_from_file(include_str!("../../assets/maps/map1.map"));
  println!("Loaded game objects.");
  let game_objects = Arc::new(Mutex::new(game_objects));

  // initiate all networking sockets
  let server_listen_address = format!("0.0.0.0:{}", SERVER_LISTEN_PORT);
  let server_send_address = format!("0.0.0.0:{}", SERVER_SEND_PORT);
  let listening_socket = UdpSocket::bind(server_listen_address.clone()).expect("Error creating listener UDP socket");
  let sending_socket = UdpSocket::bind(server_send_address).expect("Error creating sender UDP socket");
  let mut buffer = [0; 1024]; // The size of this buffer is lowkey kind of low, especially with how big the gameobject struct is.
  println!("Sockets bound.");
  println!("Listening on: {}", server_listen_address.clone());

  let mut red_team_player_count = 0;
  let mut blue_team_player_count = 0;

  // temporary
  let max_players = 4;
  
  // networking thread
  let listener_players = Arc::clone(&players);
  let listener_game_objects = Arc::clone(&game_objects);
  std::thread::spawn(move || {
    loop {
      // recieve packet
      let (amt, src) = listening_socket.recv_from(&mut buffer).expect(":(");
      let data = &buffer[..amt];
      let mut recieved_player_info: ClientPacket = bincode::deserialize(data).expect("awwww");
      // println!("SERVER: Received from {}: {:?}", src, recieved_player_info);
      
      // clean all recieved NaNs and infinites so the server doesnt explode
      recieved_player_info.aim_direction.clean();
      recieved_player_info.movement.clean();
      recieved_player_info.position.clean();
      recieved_player_info.packet_interval.clean();
      
      // update PLAYERS Vector with recieved information.
      let mut listener_players = listener_players.lock().unwrap();
      let listener_game_objects = listener_game_objects.lock().unwrap();
      let readonly_game_objects = listener_game_objects.clone();
      drop(listener_game_objects);
      
      let mut player_found: bool = false;
      
      // iterate through players
      for player_index in 0..listener_players.len() {
        
        let mut player = listener_players[player_index].clone();
        
        // use IP as identifier, check if packet from srent player correlates to our player
        if player.ip == src.ip().to_string() {
          let time_since_last_packet = recieved_player_info.packet_interval as f64;
           if time_since_last_packet < MAX_PACKET_INTERVAL &&
           time_since_last_packet > MIN_PACKET_INTERVAL  {
             // ignore this packet since it's coming in too fast
             player_found = true;
             break;
           }

          player.aim_direction = recieved_player_info.aim_direction.normalize();
          
          // Movement legality calculations
          let player_movement_speed: f32 = characters[&player.character].speed;
          player.shooting = recieved_player_info.shooting_primary;
          player.shooting_secondary = recieved_player_info.shooting_secondary;

          // check if movement is legal
          let movement_error_margin = 5.0;
          let mut movement_legal = true;

          let recieved_position = recieved_player_info.position;
          let raw_movement = recieved_player_info.movement;
          let mut movement = Vector2::new();
          movement.x = raw_movement.x * player_movement_speed * time_since_last_packet as f32;
          movement.y = raw_movement.y * player_movement_speed * time_since_last_packet as f32;
          let previous_position = player.position;

          // calculate current expected position based on input
          let mut new_position = Vector2::new();
          let mut new_movement_raw = Vector2::new();
          (new_movement_raw, _) = object_aware_movement(
            previous_position,
            raw_movement,
            movement,
            readonly_game_objects
          );

          new_position.x = previous_position.x + new_movement_raw.x * player_movement_speed * time_since_last_packet as f32;
          new_position.y = previous_position.y + new_movement_raw.y * player_movement_speed * time_since_last_packet as f32;

          if Vector2::distance(new_position, recieved_position) > movement_error_margin {
            movement_legal = false;
          }
          println!("{:?}", new_movement_raw);
          
          if movement_legal {
            // do movement.
            player.position = new_position;
            // println!("✅");
          } else {
            // Prepare for correction packet
            player.had_illegal_position = true;
            // println!("❌, {}", Vector2::distance(new_position, recieved_position));
          }
          // exit loop, and inform rest of program not to proceed with appending a new player.
          player_found = true;
          listener_players[player_index] = player;
          break
        }
      }

      // otherwise, add the player
      // NOTE: In the future this entire chunk of code will be gone, the matchmaker will populate
      // the list of players beforehand.
      if !player_found && (blue_team_player_count + red_team_player_count < max_players) {
        // decide the player's team (alternate for each player)
        let mut team: Team = Team::Blue;
        if blue_team_player_count > red_team_player_count {
          team = Team::Red;
          red_team_player_count += 1;
        } else {
          blue_team_player_count += 1;
        }
        // create server player data
        // this data is pretty irrelevant, we're just initialising the player.
        listener_players.push(ServerPlayer {
          ip: src.ip().to_string(),
          team,
          health: 255,
          position: match team {
            Team::Blue => Vector2 { x: 10.0, y: 10.0 },
            Team::Red  => Vector2 { x: 90.0, y: 90.0 },
          },
          move_direction: Vector2::new(),
          aim_direction: Vector2::new(),
          shooting: false,
          shooting_secondary: false,
          secondary_charge: 0,
          had_illegal_position: false,
          character: Character::SniperGirl,
          last_shot_time: Instant::now(),
        });
        println!("Player connected: {}", src.ip().to_string())
      }
      drop(listener_players);
    }
  });
  
  let mut server_counter: Instant = Instant::now();
  let mut delta_time: f64 = server_counter.elapsed().as_secs_f64();
  let desired_delta_time: f64 = 1.0 / 2000.0; // Hz
  let mut networking_counter: Instant = Instant::now();

  let main_game_objects = Arc::clone(&game_objects);

  // server loop.
  let characters = load_characters();
  let main_loop_players = Arc::clone(&players);
  loop {
    server_counter = Instant::now();

    let mut true_delta_time: f64 = 0.0;
    if delta_time > desired_delta_time {
      true_delta_time = delta_time;
    } else {
      true_delta_time = desired_delta_time;
    }

    
    let mut main_loop_players = main_loop_players.lock().unwrap();

    // do all logic related to players
    for player_index in 0..main_loop_players.len() {
      let shooting = main_loop_players[player_index].shooting;
      let shooting_secondary = main_loop_players[player_index].shooting_secondary;
      let last_shot_time = main_loop_players[player_index].last_shot_time;

      let character: CharacterProperties = characters[&main_loop_players[player_index].character].clone();

      if shooting && !shooting_secondary && last_shot_time.elapsed().as_secs_f32() > character.primary_cooldown {
        main_loop_players[player_index].last_shot_time = Instant::now();
        let mut game_objects = main_game_objects.lock().unwrap();
        // Do primary shooting logic
        match main_loop_players[player_index].character {
          Character::SniperGirl => {
            game_objects.push(GameObject {
              object_type: GameObjectType::SniperGirlBullet,
              position: main_loop_players[player_index].position,
              direction: main_loop_players[player_index].aim_direction,
              to_be_deleted: false,
              hitpoints: 0,
              owner_index: player_index,
              lifetime: character.primary_range / character.primary_shot_speed,
              players: Vec::new(),
              traveled_distance: 0.0,
            });
          }
          Character::HealerGirl => {
            game_objects.push(GameObject {
              object_type: GameObjectType::HealerGirlPunch,
              position: main_loop_players[player_index].position,
              direction: main_loop_players[player_index].aim_direction,
              to_be_deleted: false,
              hitpoints: 0,
              owner_index: player_index,
              lifetime: character.primary_range / character.primary_shot_speed,
              players: Vec::new(),
              traveled_distance: 0.0,
            });
          }
        }
        drop(game_objects);
      }
    }

    
    // println!("{:?}", game_objects);
    // println!("{}", 1.0 / delta_time);

    // Do all logic related to game objects
    let mut game_objects = main_game_objects.lock().unwrap();
    for game_object_index in 0..game_objects.len() {
      let game_object_type = game_objects[game_object_index].object_type;
      match game_object_type {
        GameObjectType::SniperGirlBullet => {
          (main_loop_players, *game_objects) = apply_simple_bullet_logic(main_loop_players, characters.clone(), game_objects.clone(), game_object_index, true_delta_time, false);
        }
        GameObjectType::HealerGirlPunch => {
          (main_loop_players, *game_objects) = apply_simple_bullet_logic(main_loop_players, characters.clone(), game_objects.clone(), game_object_index, true_delta_time, true);
        }
        _ => {}
      }
      game_objects[game_object_index].lifetime -= true_delta_time as f32;
      if game_objects[game_object_index].lifetime < 0.0 {
        game_objects[game_object_index].to_be_deleted = true;
      }
    }

    let mut cleansed_game_objects: Vec<GameObject> = Vec::new();
    for game_object in game_objects.clone() {
      if game_object.to_be_deleted == false {
        cleansed_game_objects.push(game_object);
      }
    }

    *game_objects = cleansed_game_objects;
    let game_objects_readonly = game_objects.clone();
    drop(game_objects);

    // Only do networking logic at some frequency
    if networking_counter.elapsed().as_secs_f64() > MAX_PACKET_INTERVAL {
      // reset the counter
      networking_counter = Instant::now();

      for (index, player) in main_loop_players.clone().iter().enumerate() {

        let mut other_players: Vec<ClientPlayer> = Vec::new();
        for (other_player_index, player) in main_loop_players.clone().iter().enumerate() {
          if other_player_index != index {
            other_players.push(ClientPlayer {
              health: player.health,
              position: player.position,
              secondary_charge: player.secondary_charge,
              aim_direction: player.aim_direction,
              movement_direction: player.move_direction,
              shooting_primary: player.shooting,
              shooting_secondary: player.shooting_secondary,
              team: player.team,
              character: player.character,
            })
          }
        }
        
        // packet sent to players
        let server_packet: ServerPacket = ServerPacket {
          player_packet_is_sent_to: ServerRecievingPlayerPacket {
            health: player.health,
            override_position: player.had_illegal_position,
            position_override: player.position,
            shooting_primary: player.shooting,
            shooting_secondary: player.shooting_secondary,
          },
          players: other_players,
          game_objects: game_objects_readonly.clone(),
        };
        main_loop_players[index].had_illegal_position = false;
        
        let mut player_ip = player.ip.clone();
        let split_player_ip: Vec<&str> = player_ip.split(":").collect();
        player_ip = split_player_ip[0].to_string();
        player_ip = format!("{}:{}", player_ip, CLIENT_LISTEN_PORT);
        // println!("PLAYER IP: {}", player_ip);
        // println!("PACKET: {:?}", server_packet);
        let serialized: Vec<u8> = bincode::serialize(&server_packet).expect("Failed to serialize message (this should never happen)");
        sending_socket.send_to(&serialized, player_ip).expect("Failed to send packet to client.");
        // player.had_illegal_position = false; // reset since we corrected the error.
      }
    }
    drop(main_loop_players);
    // println!("Server Hz: {}", 1.0 / delta_time);
    delta_time = server_counter.elapsed().as_secs_f64();
    if delta_time < desired_delta_time {
      thread::sleep(Duration::from_secs_f64(desired_delta_time - delta_time));
    }
  }
}

/// Loads any map from a properly formatted string: `<object> [posX] [posY]`
/// 
/// example:
/// ```rust
/// let game_objects: Vec<GameObject> = load_map_from_file(include_str!("../../assets/maps/map1.map"));
/// ```
/// map1.map:
/// ```
/// wall 10.0 10.0
/// wall 20.0 10.0
/// wall 30.0 10.0
/// wall 40.0 10.0
/// wall 50.0 10.0
/// ```
/// information held by server about players.
#[derive(Debug, Clone)]
pub struct ServerPlayer {
  pub ip:                            String,
  pub team:                          Team,
  pub character:                     Character,
  pub health:                        u8,
  pub position:                      Vector2,
  pub shooting:                      bool,
  pub last_shot_time:                Instant,
  pub shooting_secondary:            bool,
  pub secondary_charge:              u8,
  pub aim_direction:                 Vector2,
  pub move_direction:                Vector2,
  pub had_illegal_position:          bool,
}

/// Applies modifications to players and game objects as a result of
/// bullet behaviour this frame, for a specific bullet.
pub fn apply_simple_bullet_logic(
  mut main_loop_players: MutexGuard<Vec<ServerPlayer>>,
  characters: HashMap<Character, CharacterProperties>,
  mut game_objects: Vec<GameObject>,
  game_object_index: usize,
  true_delta_time: f64,
  pierceing_shot: bool,
) -> (MutexGuard<Vec<ServerPlayer>>, Vec<GameObject>) {
  let game_object = game_objects[game_object_index].clone();
  let player = main_loop_players[game_object.owner_index].clone();
  let character = player.character;
  let character_properties = characters[&character].clone();
  let owner_index = game_object.owner_index;
  let hit_radius: f32 = character_properties.primary_hit_radius;
  let bullet_speed: f32 = character_properties.primary_shot_speed;
  // Calculate collisions with walls
  let walls: Vec<GameObjectType> = vec![GameObjectType::Wall, GameObjectType::UnbreakableWall];
  for victim_object_index in 0..game_objects.len() {
    // if it's a wall
    if walls.contains(&game_objects[victim_object_index].object_type) {
      // if it's colliding
      if Vector2::distance(game_object.position, game_objects[victim_object_index].position) < (5.0 + hit_radius) {
        // delete the bullet
        game_objects[game_object_index].to_be_deleted = true;
        if game_objects[victim_object_index].object_type == GameObjectType::Wall {
          // damage the wall if it's not unbreakable
          if game_objects[victim_object_index].hitpoints < character_properties.primary_damage {
            game_objects[victim_object_index].to_be_deleted = true;
          } else {
            game_objects[victim_object_index].hitpoints -= character_properties.primary_damage;
          }
        }
        return (main_loop_players, game_objects); // return early
      }
    }
  }

  // Calculate collisions with players
  for player_index in 0..main_loop_players.len() {
    if Vector2::distance(game_object.position, main_loop_players[player_index].position) < hit_radius &&
    owner_index != player_index {

      if !(game_object.players.contains(&player_index)) {
        // Apply bullet damage
        if main_loop_players[player_index].team != player.team {
          if main_loop_players[player_index].health > character_properties.primary_damage {
            main_loop_players[player_index].health -= character_properties.primary_damage;
          } else {
            main_loop_players[player_index].health = 0;
          }
          game_objects[game_object_index].players.push(player_index);
        }
        // Apply bullet healing, only if in the same team
        if main_loop_players[player_index].team == player.team {
          if main_loop_players[player_index].health < character_properties.primary_heal {
            main_loop_players[player_index].health -= character_properties.primary_heal;
          } else {
            main_loop_players[player_index].health = 255;
          }
          game_objects[game_object_index].players.push(player_index);
        }
        // Apply appropriate secondary charge
        if main_loop_players[owner_index].secondary_charge < 255 - character_properties.secondary_hit_charge {
          main_loop_players[owner_index].secondary_charge += character_properties.secondary_hit_charge;
        } else {
          main_loop_players[owner_index].secondary_charge = 255;
        }
        // Destroy the bullet if it doesn't pierce.
        if !pierceing_shot {
          game_objects[game_object_index].to_be_deleted = true;
        }
      }
    }
  }
  game_objects[game_object_index].position.x += game_object.direction.x * true_delta_time as f32 * bullet_speed;
  game_objects[game_object_index].position.y += game_object.direction.y * true_delta_time as f32 * bullet_speed;
  return (main_loop_players, game_objects);
}

/// Contains extra data for bullets specifically.
#[derive(Debug, Clone)]
pub struct BulletData {
  pub hit_players: Vec<usize>,
}