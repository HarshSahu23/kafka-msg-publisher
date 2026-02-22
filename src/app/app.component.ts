import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';

/**
 * AppComponent is now a thin shell that hosts the Angular router outlet.
 * All publisher-specific logic lives in PublisherComponent.
 * Future features (e.g. Traffic Shaper) will be added as additional routes.
 */
@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet],
  template: `<router-outlet />`,
  styles: [],
})
export class AppComponent {}
