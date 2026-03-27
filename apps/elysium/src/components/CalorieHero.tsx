import React from 'react';
import { colors } from '@/theme';
import { ArcHero } from './ArcHero';

interface CalorieHeroProps {
  eaten: number;
  goal: number;
  burned?: number;
  onPress?: () => void;
}

export function CalorieHero({ eaten, goal, burned = 0, onPress }: CalorieHeroProps) {
  return (
    <ArcHero
      current={eaten}
      goal={goal + burned}
      unit="ккал"
      color={colors.accent}
      onPress={onPress}
    />
  );
}
