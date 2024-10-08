# Game design document

This document specifies all design rules and choices for this videogame.

## Basic game description

A multiplayer arena fighting game where players fight with their pick amongst a cast of varied characters. It focuses on the following values:
- Having balanced characters
- Having relatively simple but unique characters
- Having snappy movement (WASD instead of click-to-move)
- Being ontroller compatible
- Being a top-down shooter
- Having hand-drawn but 3D-looking graphics

## Base mechanics

The player has four game controls, which are independent of their picked character:
- aim
- move
- shoot
- shoot secondary

A player may move freely if unobstructed by the map, aim and shoot freely at all times, and shoot their secondary
once it is charged. These are God given rights and may not be inhibited by other players and their abilities.

## Character balance rules
Goal: balance logically/mathematically, and organically later only if needed
- All characters of the same class must take the exact same amount of time to take down a full-health same-class character when continuously shooting.
- Secondaries are never direct attacks, and usually will not combo (directly) with the primary attack.
- Healers can heal 0-100% at half the kill speed of all other characters.
- Only healers can have offensive secondaries, charged by healing teammates or passively.
Of course all of this will be kept or trashed depending on playtester opinions.